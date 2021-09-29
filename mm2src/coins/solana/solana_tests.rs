use super::*;
use crate::solana::SolanaCoin;
use crate::solana::{SolanaCoinImpl, SolanaCoinType};
use crate::MarketCoinOps;
use base58::ToBase58;
use bip39::Language;
use common::mm_ctx::{MmArc, MmCtxBuilder};
use common::privkey::key_pair_from_seed;
use ed25519_dalek_bip32::derivation_path::DerivationPath;
use ed25519_dalek_bip32::ExtendedSecretKey;
use solana_sdk::signature::Signer;
use std::str::FromStr;
use std::sync::Arc;

fn generate_key_pair_from_seed(seed: String) -> Keypair {
    let derivation_path = DerivationPath::from_str("m/44'/501'/0'").unwrap();
    let mnemonic = bip39::Mnemonic::from_phrase(seed.as_str(), Language::English).unwrap();
    let seed = bip39::Seed::new(&mnemonic, "");
    let seed_bytes: &[u8] = seed.as_bytes();

    let ext = ExtendedSecretKey::from_seed(seed_bytes)
        .unwrap()
        .derive(&derivation_path)
        .unwrap();
    let ref priv_key = ext.secret_key;
    let pub_key = ext.public_key();
    let pair = ed25519_dalek::Keypair {
        secret: ext.secret_key,
        public: pub_key,
    };

    solana_sdk::signature::keypair_from_seed(pair.to_bytes().as_ref()).unwrap()
}

fn generate_key_pair_from_iguana_seed(seed: String) -> Keypair {
    let key_pair = key_pair_from_seed(seed.as_str()).unwrap();
    let secret_key = ed25519_dalek::SecretKey::from_bytes(key_pair.private().secret.as_slice()).unwrap();
    let public_key = ed25519_dalek::PublicKey::from(&secret_key);
    let other_key_pair = ed25519_dalek::Keypair {
        secret: secret_key,
        public: public_key,
    };
    solana_sdk::signature::keypair_from_seed(other_key_pair.to_bytes().as_ref()).unwrap()
}

fn solana_coin_for_test(coin_type: SolanaCoinType, seed: String, ticker_spl: Option<String>) -> (MmArc, SolanaCoin) {
    let client = solana_client::rpc_client::RpcClient::new("https://api.testnet.solana.com/".parse().unwrap());
    let conf = json!({
        "coins":[
           {"coin":"SOL","name":"solana","protocol":{"type":"SOL"},"rpcport":80,"mm2":1}
        ]
    });
    let ctx = MmCtxBuilder::new().with_conf(conf.clone()).into_mm_arc();
    let ticker = match coin_type {
        SolanaCoinType::Solana => "SOL".to_string(),
        SolanaCoinType::Spl { .. } => ticker_spl.unwrap_or("USDC".to_string()),
    };

    let key_pair = generate_key_pair_from_seed(seed);
    let my_address = key_pair.pubkey().to_string();

    let solana_coin = SolanaCoin(Arc::new(SolanaCoinImpl {
        coin_type,
        decimals: 8,
        my_address,
        key_pair,
        ticker,
        _ctx: ctx.weak(),
        _required_confirmations: 1.into(),
        client,
    }));
    (ctx, solana_coin)
}

mod tests {
    use super::*;
    use solana_client::rpc_request::TokenAccountsFilter;

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_keypair_from_secp() {
        let bob_passphrase = get_passphrase!(".env.seed", "BOB_PASSPHRASE").unwrap();
        let solana_key_pair = generate_key_pair_from_iguana_seed(bob_passphrase);
        assert_eq!(
            "GMtMFbuVgjDnzsBd3LLBfM4X8RyYcDGCM92tPq2PG6B2",
            solana_key_pair.pubkey().to_string()
        );

        let other_solana_keypair = generate_key_pair_from_iguana_seed("bob passphrase".to_string());
        assert_eq!(
            "B7KMMHyc3eYguUMneXRznY1NWh91HoVA2muVJetstYKE",
            other_solana_keypair.pubkey().to_string()
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_prerequisites() {
        // same test as trustwallet
        {
            let fin = generate_key_pair_from_seed(
                "shoot island position soft burden budget tooth cruel issue economy destroy above".to_string(),
            );
            let public_address = fin.pubkey().to_string();
            let priv_key = &fin.secret().to_bytes()[..].to_base58();
            assert_eq!(public_address.len(), 44);
            assert_eq!(public_address, "2bUBiBNZyD29gP1oV6de7nxowMLoDBtopMMTGgMvjG5m");
            assert_eq!(priv_key, "F6czu7fdefbsCDH52JesQrBSJS5Sz25AkPLWFf8zUWhm");
            let client = solana_client::rpc_client::RpcClient::new("https://api.testnet.solana.com/".parse().unwrap());
            let balance = client.get_balance(&fin.pubkey()).expect("Expect to retrieve balance");
            assert_eq!(balance, 0);
        }

        {
            let key_pair = generate_key_pair_from_seed(
                "powder verify clutch illegal spider old grain curve robust fade twice sphere".to_string(),
            );
            let public_address = key_pair.pubkey().to_string();
            assert_eq!(public_address.len(), 44);
            assert_eq!(public_address, "DJ8wwseey5LEoMeMWb3tLDLywK8SecyYcqdzoVw24QpP");
            let client = solana_client::rpc_client::RpcClient::new("https://api.testnet.solana.com/".parse().unwrap());
            let balance = client
                .get_balance(&key_pair.pubkey())
                .expect("Expect to retrieve balance");
            assert_eq!(solana_sdk::native_token::lamports_to_sol(balance), 2.0);
            assert_eq!(balance, 2000000000);

            //  This will fetch all the balance from all tokens
            let token_accounts = client
                .get_token_accounts_by_owner(&key_pair.pubkey(), TokenAccountsFilter::ProgramId(spl_token::id()))
                .expect("");
            println!("{:?}", token_accounts);
            let actual_token_pubkey = solana_sdk::pubkey::Pubkey::from_str(token_accounts[0].pubkey.as_str()).unwrap();
            let amount = client.get_token_account_balance(&actual_token_pubkey).unwrap();
            assert_eq!(amount.ui_amount_string.as_str(), "1");
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_coin_creation() {
        let (_, sol_coin) = solana_coin_for_test(
            SolanaCoinType::Solana,
            "powder verify clutch illegal spider old grain curve robust fade twice sphere".to_string(),
            None,
        );
        assert_eq!(
            sol_coin.my_address().unwrap(),
            "DJ8wwseey5LEoMeMWb3tLDLywK8SecyYcqdzoVw24QpP"
        );

        let (_, sol_spl_usdc_coin) = solana_coin_for_test(
            SolanaCoinType::Spl {
                platform: "SOL".to_string(),
                token_addr: solana_sdk::pubkey::Pubkey::from_str("CpMah17kQEL2wqyMKt3mZBdTnZbkbfx4nqmQMFDP5vwp")
                    .unwrap(),
            },
            "powder verify clutch illegal spider old grain curve robust fade twice sphere".to_string(),
            Some("USDC".to_string()),
        );

        assert_eq!(
            sol_spl_usdc_coin.my_address().unwrap(),
            "DJ8wwseey5LEoMeMWb3tLDLywK8SecyYcqdzoVw24QpP"
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_my_balance() {
        let (_, sol_coin) = solana_coin_for_test(
            SolanaCoinType::Solana,
            "powder verify clutch illegal spider old grain curve robust fade twice sphere".to_string(),
            None,
        );
        let res = sol_coin.my_balance().wait().unwrap();
        assert_eq!(res.spendable, BigDecimal::from(2.0));

        let (_, sol_spl_usdc_coin) = solana_coin_for_test(
            SolanaCoinType::Spl {
                platform: "SOL".to_string(),
                token_addr: solana_sdk::pubkey::Pubkey::from_str("CpMah17kQEL2wqyMKt3mZBdTnZbkbfx4nqmQMFDP5vwp")
                    .unwrap(),
            },
            "powder verify clutch illegal spider old grain curve robust fade twice sphere".to_string(),
            Some("USDC".to_string()),
        );

        let res = sol_spl_usdc_coin.my_balance().wait().unwrap();
        assert_eq!(res.spendable, BigDecimal::from(1.0));

        let (_, sol_spl_wsol_coin) = solana_coin_for_test(
            SolanaCoinType::Spl {
                platform: "SOL".to_string(),
                token_addr: solana_sdk::pubkey::Pubkey::from_str("So11111111111111111111111111111111111111112")
                    .unwrap(),
            },
            "powder verify clutch illegal spider old grain curve robust fade twice sphere".to_string(),
            Some("WSOL".to_string()),
        );
        let res = sol_spl_wsol_coin.my_balance().wait().unwrap();
        assert_eq!(res.spendable, BigDecimal::from(0.0));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_block_height() {
        let (_, sol_coin) = solana_coin_for_test(
            SolanaCoinType::Solana,
            "powder verify clutch illegal spider old grain curve robust fade twice sphere".to_string(),
            None,
        );
        let res = sol_coin.current_block().wait().unwrap();
        println!("block is : {}", res);
        assert!(res > 0);
    }
}