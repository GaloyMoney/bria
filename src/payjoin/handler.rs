//! Payjoin handler implementation for Bria
//! Implements BIP78 Payjoin receiver logic

use anyhow::Result;
use bdk::bitcoin::psbt::PartiallySignedTransaction;
use bdk::bitcoin::{Sequence, Witness};
use bdk::bitcoin::OutPoint;
use bdk::Wallet as BdkWallet;
use sqlx::PgPool;
use std::collections::HashSet;
use std::str::FromStr;

use crate::primitives::{bitcoin, WalletId};
use crate::utxo::Utxos;
use crate::wallet::{Wallets, Wallet};

pub struct PayjoinProposal {
    pub original_psbt: Vec<u8>,
    pub payjoin_psbt: Vec<u8>,
    // Removed the pool field, as it is not needed in the proposal struct
}

pub struct PayjoinHandler;

impl PayjoinHandler {
    pub async fn propose_payjoin(
        &self,
        wallet_id: String,
        original_psbt: Vec<u8>,
        pool: PgPool, // Pass the DB pool from the application context
    ) -> Result<PayjoinProposal> {
        // 1. Parse the PSBT
        let psbt: PartiallySignedTransaction = bdk::bitcoin::psbt::PartiallySignedTransaction::deserialize(&original_psbt)
            .map_err(|e| anyhow::anyhow!("Invalid PSBT: {e}"))?;

        // 2. Load the wallet
        let wallet_uuid = WalletId::from_str(&wallet_id)?;
        let wallets = Wallets::new(&pool);
        let wallet: Wallet = wallets.find_by_id(wallet_uuid).await?;

        // 3. List available UTXOs for all keychains
        let utxos = Utxos::new(&pool);
        let keychain_ids: Vec<_> = wallet.keychain_ids().collect();
        let keychain_utxos = utxos.find_keychain_utxos(keychain_ids.clone().into_iter()).await?;
        let mut available_utxos = vec![];
        for (_keychain_id, kc_utxos) in keychain_utxos {
            available_utxos.extend(kc_utxos.utxos);
        }

        // 4. Select a UTXO not already in the PSBT
        let used_outpoints: HashSet<OutPoint> = psbt.unsigned_tx.input.iter().map(|i| i.previous_output).collect();
        let payjoin_utxo = available_utxos
            .into_iter()
            .find(|utxo| !used_outpoints.contains(&utxo.outpoint))
            .ok_or_else(|| anyhow::anyhow!("No suitable UTXO for payjoin"))?;

        // 5. Add the UTXO as an input to the PSBT (BDK logic)
        let mut new_psbt = psbt.clone();
        // Add the selected UTXO as a new input
        let new_input = bdk::bitcoin::psbt::Input {
            witness_utxo: Some(bdk::bitcoin::TxOut {
                value: payjoin_utxo.value.into(),
                script_pubkey: payjoin_utxo.address.as_ref().map(|a| a.script_pubkey()).unwrap_or_default(),
            }),
            ..Default::default()
        };
        let prevout = payjoin_utxo.outpoint;
        new_psbt.inputs.push(new_input);
        new_psbt.unsigned_tx.input.push(bdk::bitcoin::TxIn {
            previous_output: prevout,
            script_sig: bdk::bitcoin::ScriptBuf::new(),
            sequence: Sequence(0xFFFFFFFF),
            witness: Witness::default(),
        });
        // Optionally, you may want to adjust outputs (e.g., increase receiver's output by the UTXO value minus fee)
        // For a minimal implementation, just add the input and let sender finalize change.

        // 6. Return the new PSBT
        Ok(PayjoinProposal {
            original_psbt,
            // Use the PSBT's serialize method instead of bitcoin::consensus::encode::serialize
            payjoin_psbt: new_psbt.serialize(),
        })
    }
}
