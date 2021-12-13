use crate::platform_coin_with_tokens::{EnablePlatformCoinWithTokensError, PlatformWithTokensActivationOps,
                                       RegisterTokenInfo, TokenActivationParams, TokenActivationRequest,
                                       TokenAsMmCoinInitializer, TokenInitializer, TokenOf};
use crate::prelude::*;
use crate::slp_token_activation::SlpActivationRequest;
use async_trait::async_trait;
use coins::utxo::bch::{bch_coin_from_conf_and_params, BchActivationRequest, BchCoin, CashAddrPrefix};
use coins::utxo::rpc_clients::UtxoRpcError;
use coins::utxo::slp::{SlpProtocolConf, SlpToken};
use coins::utxo::UtxoCommonOps;
use coins::{CoinBalance, CoinProtocol, MarketCoinOps, MmCoin};
use common::mm_ctx::MmArc;
use common::mm_error::prelude::*;
use common::Future01CompatExt;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::HashMap;
use std::str::FromStr;

pub struct SlpTokenInitializer {
    platform_coin: BchCoin,
}

impl TokenOf for SlpToken {
    type PlatformCoin = BchCoin;
}

#[async_trait]
impl TokenInitializer for SlpTokenInitializer {
    type Token = SlpToken;
    type TokenActivationRequest = SlpActivationRequest;
    type TokenProtocol = SlpProtocolConf;
    type InitTokensError = std::convert::Infallible;

    fn tokens_requests_from_platform_request(
        platform_params: &BchWithTokensActivationRequest,
    ) -> Vec<TokenActivationRequest<Self::TokenActivationRequest>> {
        platform_params.slp_tokens_requests.clone()
    }

    async fn init_tokens(
        &self,
        activation_params: Vec<TokenActivationParams<SlpActivationRequest, SlpProtocolConf>>,
    ) -> Result<Vec<SlpToken>, MmError<std::convert::Infallible>> {
        let tokens = activation_params
            .into_iter()
            .map(|params| {
                // confirmation settings from RPC request have the highest priority
                let required_confirmations = params.activation_request.required_confirmations.unwrap_or_else(|| {
                    params
                        .protocol
                        .required_confirmations
                        .unwrap_or_else(|| self.platform_coin.required_confirmations())
                });

                SlpToken::new(
                    params.protocol.decimals,
                    params.ticker,
                    params.protocol.token_id,
                    self.platform_coin.clone(),
                    required_confirmations,
                )
            })
            .collect();

        Ok(tokens)
    }

    fn platform_coin(&self) -> &BchCoin { &self.platform_coin }
}

impl RegisterTokenInfo<SlpToken> for BchCoin {
    fn register_token_info(&self, token: &SlpToken) { self.add_slp_token_info(token.ticker().into(), token.get_info()) }
}

impl From<BchWithTokensActivationError> for EnablePlatformCoinWithTokensError {
    fn from(err: BchWithTokensActivationError) -> Self {
        match err {
            BchWithTokensActivationError::PlatformCoinCreationError { ticker, error } => {
                EnablePlatformCoinWithTokensError::PlatformCoinCreationError { ticker, error }
            },
            BchWithTokensActivationError::InvalidSlpPrefix { ticker, prefix, error } => {
                EnablePlatformCoinWithTokensError::Internal(format!(
                    "Invalid slp prefix {} configured for {}. Error: {}",
                    prefix, ticker, error
                ))
            },
            BchWithTokensActivationError::Transport(e) => EnablePlatformCoinWithTokensError::Transport(e),
            BchWithTokensActivationError::Internal(e) => EnablePlatformCoinWithTokensError::Internal(e),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct BchWithTokensActivationRequest {
    #[serde(flatten)]
    platform_request: BchActivationRequest,
    slp_tokens_requests: Vec<TokenActivationRequest<SlpActivationRequest>>,
}

pub struct BchProtocolInfo {
    slp_prefix: String,
}

impl TryFromCoinProtocol for BchProtocolInfo {
    fn try_from_coin_protocol(proto: CoinProtocol) -> Result<Self, MmError<CoinProtocol>>
    where
        Self: Sized,
    {
        match proto {
            CoinProtocol::BCH { slp_prefix } => Ok(BchProtocolInfo { slp_prefix }),
            protocol => MmError::err(protocol),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BchWithTokensActivationResult {
    current_block: u64,
    bch_addresses_infos: HashMap<String, CoinAddressInfo<CoinBalance>>,
    slp_addresses_infos: HashMap<String, CoinAddressInfo<TokenBalances>>,
}

#[derive(Debug)]
pub enum BchWithTokensActivationError {
    PlatformCoinCreationError {
        ticker: String,
        error: String,
    },
    InvalidSlpPrefix {
        ticker: String,
        prefix: String,
        error: String,
    },
    Transport(String),
    Internal(String),
}

impl From<UtxoRpcError> for BchWithTokensActivationError {
    fn from(err: UtxoRpcError) -> Self { BchWithTokensActivationError::Transport(err.to_string()) }
}

#[async_trait]
impl PlatformWithTokensActivationOps for BchCoin {
    type ActivationRequest = BchWithTokensActivationRequest;
    type PlatformProtocolInfo = BchProtocolInfo;
    type ActivationResult = BchWithTokensActivationResult;
    type ActivationError = BchWithTokensActivationError;

    async fn init_platform_coin(
        ctx: MmArc,
        ticker: String,
        platform_conf: Json,
        activation_request: Self::ActivationRequest,
        protocol_conf: Self::PlatformProtocolInfo,
        priv_key: &[u8],
    ) -> Result<Self, MmError<Self::ActivationError>> {
        let slp_prefix = CashAddrPrefix::from_str(&protocol_conf.slp_prefix).map_to_mm(|error| {
            BchWithTokensActivationError::InvalidSlpPrefix {
                ticker: ticker.clone(),
                prefix: protocol_conf.slp_prefix,
                error,
            }
        })?;

        let platform_coin = bch_coin_from_conf_and_params(
            &ctx,
            &ticker,
            &platform_conf,
            activation_request.platform_request,
            slp_prefix,
            priv_key,
        )
        .await
        .map_to_mm(|error| BchWithTokensActivationError::PlatformCoinCreationError { ticker, error })?;
        Ok(platform_coin)
    }

    fn token_initializers(
        &self,
    ) -> Vec<Box<dyn TokenAsMmCoinInitializer<PlatformCoin = Self, ActivationRequest = Self::ActivationRequest>>> {
        vec![Box::new(SlpTokenInitializer {
            platform_coin: self.clone(),
        })]
    }

    async fn get_activation_result(
        &self,
    ) -> Result<BchWithTokensActivationResult, MmError<BchWithTokensActivationError>> {
        let my_address = &self.as_ref().my_address;
        let my_slp_address = self
            .get_my_slp_address()
            .map_to_mm(BchWithTokensActivationError::Internal)?
            .encode()
            .map_to_mm(BchWithTokensActivationError::Internal)?;

        let current_block = self.as_ref().rpc_client.get_block_count().compat().await?;

        let bch_unspents = self.bch_unspents_for_display(my_address).await?;
        let bch_balance = bch_unspents.platform_balance(self.decimals());

        let mut token_balances = HashMap::new();
        for (token_ticker, info) in self.get_slp_tokens_infos().iter() {
            let token_balance = bch_unspents.slp_token_balance(&info.token_id, info.decimals);
            token_balances.insert(token_ticker.clone(), token_balance);
        }

        let mut result = BchWithTokensActivationResult {
            current_block,
            bch_addresses_infos: HashMap::new(),
            slp_addresses_infos: HashMap::new(),
        };

        result
            .bch_addresses_infos
            .insert(my_address.to_string(), CoinAddressInfo {
                derivation_method: DerivationMethod::Iguana,
                pubkey: self.my_public_key().to_string(),
                balances: bch_balance,
            });

        result.slp_addresses_infos.insert(my_slp_address, CoinAddressInfo {
            derivation_method: DerivationMethod::Iguana,
            pubkey: self.my_public_key().to_string(),
            balances: token_balances,
        });
        Ok(result)
    }
}