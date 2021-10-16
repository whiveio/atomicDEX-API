use super::*;
use crate::solana::solana_common_tests::{generate_key_pair_from_iguana_seed, generate_key_pair_from_seed,
                                         solana_coin_for_test, SolanaNet};
use crate::solana::solana_transaction_transfer::SolanaTransactionTransfer;
use crate::MarketCoinOps;
use base58::ToBase58;
use solana_client::rpc_request::TokenAccountsFilter;
use solana_sdk::{signature::Signature, signature::Signer};
use solana_transaction_status::UiTransactionEncoding;
use std::str;
use std::str::FromStr;

mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_keypair_from_secp() {
        let solana_key_pair = generate_key_pair_from_iguana_seed("federal stay trigger hour exist success game vapor become comfort action phone bright ill target wild nasty crumble dune close rare fabric hen iron".to_string());
        assert_eq!(
            "FJktmyjV9aBHEShT4hfnLpr9ELywdwVtEL1w1rSWgbVf",
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
            let key_pair = generate_key_pair_from_iguana_seed("passphrase not really secure".to_string());
            let public_address = key_pair.pubkey().to_string();
            assert_eq!(public_address.len(), 44);
            assert_eq!(public_address, "2jTgfhf98GosnKSCXjL5YSiEa3MLrmR42yy9kZZq1i2c");
            let client = solana_client::rpc_client::RpcClient::new("https://api.testnet.solana.com/".parse().unwrap());
            let balance = client
                .get_balance(&key_pair.pubkey())
                .expect("Expect to retrieve balance");
            assert_eq!(solana_sdk::native_token::lamports_to_sol(balance), 0.0);
            assert_eq!(balance, 0);

            //  This will fetch all the balance from all tokens
            let token_accounts = client
                .get_token_accounts_by_owner(&key_pair.pubkey(), TokenAccountsFilter::ProgramId(spl_token::id()))
                .expect("");
            println!("{:?}", token_accounts);
            let actual_token_pubkey = solana_sdk::pubkey::Pubkey::from_str(token_accounts[0].pubkey.as_str()).unwrap();
            let amount = client.get_token_account_balance(&actual_token_pubkey).unwrap();
            assert_ne!(amount.ui_amount_string.as_str(), "0");
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_coin_creation() {
        let passphrase = "federal stay trigger hour exist success game vapor become comfort action phone bright ill target wild nasty crumble dune close rare fabric hen iron".to_string();
        let (_, sol_coin) = solana_coin_for_test(passphrase.clone(), SolanaNet::Testnet);
        assert_eq!(
            sol_coin.my_address().unwrap(),
            "FJktmyjV9aBHEShT4hfnLpr9ELywdwVtEL1w1rSWgbVf"
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_my_balance() {
        let passphrase = "federal stay trigger hour exist success game vapor become comfort action phone bright ill target wild nasty crumble dune close rare fabric hen iron".to_string();
        let (_, sol_coin) = solana_coin_for_test(passphrase.clone(), SolanaNet::Testnet);
        let res = sol_coin.my_balance().wait().unwrap();
        assert_ne!(res.spendable, BigDecimal::from(0.0));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_block_height() {
        let passphrase = "federal stay trigger hour exist success game vapor become comfort action phone bright ill target wild nasty crumble dune close rare fabric hen iron".to_string();
        let (_, sol_coin) = solana_coin_for_test(passphrase.clone(), SolanaNet::Testnet);
        let res = sol_coin.current_block().wait().unwrap();
        println!("block is : {}", res);
        assert!(res > 0);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_validate_address() {
        let passphrase = "federal stay trigger hour exist success game vapor become comfort action phone bright ill target wild nasty crumble dune close rare fabric hen iron".to_string();
        let (_, sol_coin) = solana_coin_for_test(passphrase.clone(), SolanaNet::Testnet);

        // invalid len
        let res = sol_coin.validate_address("invalidaddressobviously");
        assert_eq!(res.is_valid, false);

        let res = sol_coin.validate_address("GMtMFbuVgjDnzsBd3LLBfM4X8RyYcDGCM92tPq2PG6B2");
        assert_eq!(res.is_valid, true);

        // Typo
        let res = sol_coin.validate_address("Fr8fraJXAe1cFU81mF7NhHTrUzXjZAJkQE1gUQ11riH");
        assert_eq!(res.is_valid, false);

        // invalid len
        let res = sol_coin.validate_address("r8fraJXAe1cFU81mF7NhHTrUzXjZAJkQE1gUQ11riHn");
        assert_eq!(res.is_valid, false);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_test_transactions() {
        let passphrase = "federal stay trigger hour exist success game vapor become comfort action phone bright ill target wild nasty crumble dune close rare fabric hen iron".to_string();
        let (_, sol_coin) = solana_coin_for_test(passphrase.clone(), SolanaNet::Devnet);
        let valid_tx_details = sol_coin
            .withdraw(WithdrawRequest {
                coin: "SOL".to_string(),
                to: sol_coin.my_address.clone(),
                amount: BigDecimal::from(0.0001),
                max: false,
                fee: None,
            })
            .wait()
            .unwrap();
        assert_eq!(valid_tx_details.total_amount, BigDecimal::from(0.0001));
        assert_eq!(valid_tx_details.coin, "SOL".to_string());
        assert_eq!(valid_tx_details.received_by_me, BigDecimal::from(0.0001));
        assert_ne!(valid_tx_details.timestamp, 0);

        let invalid_tx = sol_coin
            .withdraw(WithdrawRequest {
                coin: "SOL".to_string(),
                to: sol_coin.my_address.clone(),
                amount: BigDecimal::from(10),
                max: false,
                fee: None,
            })
            .wait();

        // NotSufficientBalance
        assert_eq!(invalid_tx.is_err(), true);

        let tx_str = str::from_utf8(&*valid_tx_details.tx_hex.0).unwrap();
        let res = sol_coin.send_raw_tx(tx_str).wait();
        assert_eq!(res.is_err(), false);
    }

    // This test is just a unit test for brainstorming around tx_history for base_coin.
    #[test]
    #[ignore]
    #[cfg(not(target_arch = "wasm32"))]
    fn solana_test_tx_history() {
        let passphrase = "federal stay trigger hour exist success game vapor become comfort action phone bright ill target wild nasty crumble dune close rare fabric hen iron".to_string();
        let (_, sol_coin) = solana_coin_for_test(passphrase.clone(), SolanaNet::Devnet);
        let res = sol_coin
            .client
            .get_signatures_for_address(&sol_coin.key_pair.pubkey())
            .unwrap();
        //println!("{:?}", res);
        let mut history = Vec::new();
        for cur in res.iter() {
            let signature = Signature::from_str(cur.signature.clone().as_str()).unwrap();
            let res = sol_coin
                .client
                .get_transaction(&signature, UiTransactionEncoding::JsonParsed)
                .unwrap();
            let str = serde_json::to_string(&res).unwrap();
            //println!("{}", str);
            let tx_solana_transfer: SolanaTransactionTransfer = serde_json::from_str(str.as_str()).unwrap();
            let tx_info = tx_solana_transfer.extract_first_transfer_instructions().unwrap();
            let tx = TransactionDetails {
                tx_hex: Default::default(),
                tx_hash: signature.as_ref().into(),
                from: vec![tx_info.source],
                to: vec![tx_info.destination],
                total_amount: BigDecimal::from(lamports_to_sol(tx_info.lamports)),
                spent_by_me: Default::default(),
                received_by_me: Default::default(),
                my_balance_change: Default::default(),
                block_height: tx_solana_transfer.slot,
                timestamp: tx_solana_transfer.block_time,
                fee_details: Some(
                    SolanaFeeDetails {
                        amount: lamports_to_sol(tx_solana_transfer.meta.fee).into(),
                    }
                    .into(),
                ),
                coin: sol_coin.ticker.clone(),
                internal_id: Default::default(),
                kmd_rewards: None,
            };
            println!("signature: {}", signature.to_string());
            println!("tx: {}", serde_json::to_string(&tx).unwrap());
            history.push(tx);
        }

        println!("{}", serde_json::to_string(&history).unwrap());
    }
}
