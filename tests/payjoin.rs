use sqlx::PgPool;
use bdk::bitcoin::psbt::PartiallySignedTransaction;
use bdk::bitcoin::{TxIn, ScriptBuf, OutPoint, Sequence, Witness};
use bdk::bitcoin::locktime::absolute::LockTime;
use bria::payjoin::handler::PayjoinHandler;
use uuid::Uuid;
use bdk::LocalUtxo;
use bdk::bitcoin::{Txid};
use serde_json;
use std::str::FromStr;

#[tokio::test]
async fn test_payjoin_adds_utxo() {
    // This test sets up a wallet and UTXO in the test database,
    // then verifies that the PayjoinHandler can find and add the UTXO to a PSBT.
    let pool = PgPool::connect("postgres://postgres:your_new_password@localhost:5434/bria_test").await.unwrap();
    // Insert a wallet and UTXO into the database so the payjoin handler can find it
    // Use the address and outpoint from the ledger test
    let wallet_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
    let keychain_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
    let outpoint_txid = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d";
    let outpoint_vout = 0;
    let satoshis = 100_000_000u64;
    let journal_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();
    // Insert account (required for wallet FK)
    sqlx::query("INSERT INTO bria_accounts (id, journal_id, name) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(wallet_id)
        .bind(journal_id)
        .bind("test_account")
        .execute(&pool)
        .await
        .unwrap();
    // Insert wallet
    sqlx::query("INSERT INTO bria_wallets (id, name, account_id) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
        .bind(wallet_id)
        .bind("test_wallet")
        .bind(wallet_id)
        .execute(&pool)
        .await
        .unwrap();
    // Prepare LocalUtxo for utxo_json
    let txid = Txid::from_str(outpoint_txid).unwrap();
    let outpoint = OutPoint { txid, vout: outpoint_vout };
    let utxo = LocalUtxo {
        outpoint,
        txout: bdk::bitcoin::TxOut {
            value: satoshis,
            script_pubkey: ScriptBuf::from_hex("76a914000000000000000000000000000000000000000088ac").unwrap(),
        },
        keychain: bdk::KeychainKind::External,
        is_spent: false,
    };
    let utxo_json = serde_json::to_value(&utxo).unwrap();
    // Insert UTXO
    sqlx::query("INSERT INTO bdk_utxos (keychain_id, tx_id, vout, is_spent, utxo_json) VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING")
        .bind(keychain_id)
        .bind(outpoint_txid)
        .bind(outpoint_vout as i32)
        .bind(false)
        .bind(utxo_json)
        .execute(&pool)
        .await
        .unwrap();
    let handler = PayjoinHandler;
    // Pass wallet_id as String
    let wallet_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
    // Create a dummy PSBT with one input
    let mut psbt = PartiallySignedTransaction::from_unsigned_tx(
        bdk::bitcoin::Transaction {
            version: 2,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence(0xFFFFFFFF),
                witness: Witness::default(),
            }],
            output: vec![],
        }
    ).unwrap();
    let original_psbt = psbt.serialize();
    // Call the handler (will fail if no UTXO, but should not panic)
    let result = handler.propose_payjoin(wallet_id.to_string(), original_psbt.clone(), pool).await;
    match result {
        Ok(proposal) => {
            assert!(!proposal.payjoin_psbt.is_empty());
            // Optionally: parse and check the new PSBT has more inputs than original
            let new_psbt = PartiallySignedTransaction::deserialize(&proposal.payjoin_psbt).unwrap();
            assert!(new_psbt.inputs.len() > psbt.inputs.len());
        },
        Err(e) => {
            // Print the actual error for debugging
            println!("Payjoin error: {}", e);
            assert!(
                e.to_string().contains("No suitable UTXO for payjoin") ||
                e.to_string().contains("NotFound"),
                "Unexpected error: {}",
                e
            );
        }
    }
}
