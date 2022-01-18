use super::*;
use bitcoin::blockdata::script::Script;
use bitcoin::blockdata::transaction::Transaction;
use common::executor::{spawn, Timer};
use common::log;
use core::time::Duration;
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning::chain::keysinterface::SpendableOutputDescriptor;
use lightning::util::events::{Event, EventHandler, PaymentPurpose};
use parking_lot::Mutex as PaMutex;
use rand::Rng;
use script::{Builder, SignatureVersion};
use secp256k1::Secp256k1;
use std::collections::hash_map::Entry;
use std::convert::TryFrom;
use std::sync::Arc;
use utxo_signer::with_key_pair::sign_tx;

pub struct LightningEventHandler {
    filter: Arc<PlatformFields>,
    channel_manager: Arc<ChannelManager>,
    keys_manager: Arc<KeysManager>,
    inbound_payments: Arc<PaMutex<HashMap<PaymentHash, PaymentInfo>>>,
    outbound_payments: Arc<PaMutex<HashMap<PaymentHash, PaymentInfo>>>,
}

impl EventHandler for LightningEventHandler {
    // TODO: Implement all the cases
    fn handle_event(&self, event: &Event) {
        match event {
            Event::FundingGenerationReady {
                temporary_channel_id,
                output_script,
                user_channel_id,
                ..
            } => {
                log::info!(
                    "Handling FundingGenerationReady event for temporary_channel_id: {}",
                    hex::encode(temporary_channel_id)
                );
                self.handle_funding_generation_ready(*temporary_channel_id, output_script, *user_channel_id);
            },
            Event::PaymentReceived {
                payment_hash,
                amt,
                purpose,
            } => {
                log::info!(
                    "Handling PaymentReceived event for payment_hash: {}",
                    hex::encode(payment_hash.0)
                );
                self.handle_payment_received(*payment_hash, *amt, purpose);
            },
            Event::PaymentSent {
                payment_preimage,
                payment_hash,
                ..
            } => {
                log::info!(
                    "Handling PaymentSent event for payment_hash: {}",
                    hex::encode(payment_hash.0)
                );
                self.handle_payment_sent(*payment_preimage, *payment_hash);
            },
            Event::PaymentPathFailed { payment_hash, .. } => log::info!(
                "Handling PaymentPathFailed event for payment_hash: {}",
                hex::encode(payment_hash.0)
            ),
            Event::PaymentFailed { payment_hash, .. } => {
                log::info!(
                    "Handling PaymentFailed event for payment_hash: {}",
                    hex::encode(payment_hash.0)
                );
                self.handle_payment_failed(payment_hash);
            },
            Event::PendingHTLCsForwardable { time_forwardable } => {
                log::info!("Handling PendingHTLCsForwardable event!");
                self.handle_pending_htlcs_forwards(*time_forwardable);
            },
            Event::SpendableOutputs { outputs } => {
                log::info!("Handling SpendableOutputs event!");
                self.handle_spendable_outputs(outputs)
            },
            Event::PaymentForwarded { .. } => log::info!("Handling PaymentForwarded event!"),
            Event::ChannelClosed { channel_id, reason, .. } => {
                // Todo: Use storage to store channels history
                log::info!(
                    "Channel: {} closed for the following reason: {}",
                    hex::encode(channel_id),
                    reason
                )
            },
            Event::DiscardFunding { channel_id, .. } => {
                log::info!("Handling DiscardFunding event for channel: {}", hex::encode(channel_id))
            },
            Event::PaymentPathSuccessful {
                payment_id,
                payment_hash,
                ..
            } => log::info!(
                "Handling PaymentPathSuccessful event for payment_hash: {}, payment_id: {}",
                hex::encode(payment_hash.map(|h| hex::encode(h.0)).unwrap_or_default()),
                hex::encode(payment_id.0)
            ),
        }
    }
}

// Generates the raw funding transaction with one output equal to the channel value.
fn sign_funding_transaction(
    request_id: u64,
    output_script: &Script,
    filter: Arc<PlatformFields>,
) -> OpenChannelResult<Transaction> {
    let coin = &filter.platform_coin;
    let mut unsigned = {
        let unsigned_funding_txs = filter.unsigned_funding_txs.lock();
        unsigned_funding_txs
            .get(&request_id)
            .ok_or_else(|| {
                OpenChannelError::InternalError(format!("Unsigned funding tx not found for request id: {}", request_id))
            })?
            .clone()
    };
    unsigned.outputs[0].script_pubkey = output_script.to_bytes().into();

    let my_address = coin.as_ref().derivation_method.iguana_or_err()?;
    let key_pair = coin.as_ref().priv_key_policy.key_pair_or_err()?;

    let prev_script = Builder::build_p2pkh(&my_address.hash);
    let signed = sign_tx(
        unsigned,
        key_pair,
        prev_script,
        SignatureVersion::WitnessV0,
        coin.as_ref().conf.fork_id,
    )?;

    Transaction::try_from(signed).map_to_mm(|e| OpenChannelError::ConvertTxErr(e.to_string()))
}

impl LightningEventHandler {
    pub fn new(
        filter: Arc<PlatformFields>,
        channel_manager: Arc<ChannelManager>,
        keys_manager: Arc<KeysManager>,
        inbound_payments: Arc<PaMutex<HashMap<PaymentHash, PaymentInfo>>>,
        outbound_payments: Arc<PaMutex<HashMap<PaymentHash, PaymentInfo>>>,
    ) -> Self {
        LightningEventHandler {
            filter,
            channel_manager,
            keys_manager,
            inbound_payments,
            outbound_payments,
        }
    }

    fn handle_funding_generation_ready(
        &self,
        temporary_channel_id: [u8; 32],
        output_script: &Script,
        user_channel_id: u64,
    ) {
        let funding_tx = match sign_funding_transaction(user_channel_id, output_script, self.filter.clone()) {
            Ok(tx) => tx,
            Err(e) => {
                log::error!(
                    "Error generating funding transaction for temporary channel id {:?}: {}",
                    temporary_channel_id,
                    e.to_string()
                );
                // TODO: use issue_channel_close_events here when implementing channel closure this will push a Event::DiscardFunding
                // event for the other peer
                return;
            },
        };
        // Give the funding transaction back to LDK for opening the channel.
        if let Err(e) = self
            .channel_manager
            .funding_transaction_generated(&temporary_channel_id, funding_tx)
        {
            log::error!("{:?}", e);
        }
    }

    fn handle_payment_received(&self, payment_hash: PaymentHash, amt: u64, purpose: &PaymentPurpose) {
        let (payment_preimage, payment_secret) = match purpose {
            PaymentPurpose::InvoicePayment {
                payment_preimage,
                payment_secret,
            } => match payment_preimage {
                Some(preimage) => (*preimage, Some(*payment_secret)),
                None => return,
            },
            PaymentPurpose::SpontaneousPayment(preimage) => (*preimage, None),
        };
        let status = match self.channel_manager.claim_funds(payment_preimage) {
            true => {
                log::info!(
                    "Received an amount of {} millisatoshis for payment hash {}",
                    amt,
                    hex::encode(payment_hash.0)
                );
                HTLCStatus::Succeeded
            },
            false => HTLCStatus::Failed,
        };
        let mut payments = self.inbound_payments.lock();
        match payments.entry(payment_hash) {
            Entry::Occupied(mut e) => {
                let payment = e.get_mut();
                payment.status = status;
                payment.preimage = Some(payment_preimage);
                payment.secret = payment_secret;
            },
            Entry::Vacant(e) => {
                e.insert(PaymentInfo {
                    preimage: Some(payment_preimage),
                    secret: payment_secret,
                    status,
                    amt_msat: Some(amt),
                });
            },
        }
    }

    fn handle_payment_sent(&self, payment_preimage: PaymentPreimage, payment_hash: PaymentHash) {
        let mut outbound_payments = self.outbound_payments.lock();
        for (hash, payment) in outbound_payments.iter_mut() {
            if *hash == payment_hash {
                payment.preimage = Some(payment_preimage);
                payment.status = HTLCStatus::Succeeded;
                log::info!(
                    "Successfully sent payment of {} millisatoshis with payment hash {}",
                    payment.amt_msat.unwrap_or_default(),
                    hex::encode(payment_hash.0)
                );
            }
        }
    }

    fn handle_payment_failed(&self, payment_hash: &PaymentHash) {
        let mut outbound_payments = self.outbound_payments.lock();
        let outbound_payment = outbound_payments.get_mut(payment_hash);
        if let Some(payment) = outbound_payment {
            payment.status = HTLCStatus::Failed;
        }
    }

    fn handle_pending_htlcs_forwards(&self, time_forwardable: Duration) {
        let min_wait_time = time_forwardable.as_millis() as u32;
        let channel_manager = self.channel_manager.clone();
        spawn(async move {
            let millis_to_sleep = rand::thread_rng().gen_range(min_wait_time, min_wait_time * 5);
            Timer::sleep_ms(millis_to_sleep).await;
            channel_manager.process_pending_htlc_forwards();
        });
    }

    fn handle_spendable_outputs(&self, outputs: &[SpendableOutputDescriptor]) {
        let platform_coin = &self.filter.platform_coin;
        // Todo: add support for Hardware wallets for funding transactions and spending spendable outputs (channel closing transactions)
        let my_address = match platform_coin.as_ref().derivation_method.iguana_or_err() {
            Ok(addr) => addr,
            Err(e) => {
                log::error!("{}", e);
                return;
            },
        };
        let change_destination_script = Builder::build_witness_script(&my_address.hash).to_bytes().take().into();
        let feerate_sat_per_1000_weight = platform_coin.get_est_sat_per_1000_weight(ConfirmationTarget::Normal);
        let output_descriptors = &outputs.iter().collect::<Vec<_>>();
        let spending_tx = match self.keys_manager.spend_spendable_outputs(
            output_descriptors,
            Vec::new(),
            change_destination_script,
            feerate_sat_per_1000_weight,
            &Secp256k1::new(),
        ) {
            Ok(tx) => tx,
            Err(_) => {
                log::error!("Error spending spendable outputs");
                return;
            },
        };
        platform_coin.broadcast_transaction(&spending_tx);
    }
}
