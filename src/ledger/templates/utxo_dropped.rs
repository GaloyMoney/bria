use bdk::BlockTime;
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use super::shared_meta::*;
use crate::{
    ledger::{constants::*, error::LedgerError},
    primitives::*,
};

pub struct UtxoDropped {}

impl UtxoDropped {
    #[instrument(name = "ledger.utxo_dropped.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), LedgerError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .metadata("params.meta")
            .description("'Onchain tx dropped'")
            .build()
            .expect("Couldn't build TxInput");

        let entries = vec![
            // Reverse the previous LOGICAL entries
            EntryInput::builder()
                .entry_type("'UTXO_DROPPED_LOG_IN_PEN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_DROPPED_LOG_IN_PEN_DR'")
                .currency("'BTC'")
                .account_id("params.effective_incoming_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            // Reverse the previous FEE entries
            EntryInput::builder()
                .entry_type("'UTXO_DROPPED_FR_ENC_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_fee_account_id")
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_spending_fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_DROPPED_FR_ENC_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_FEE_ID}')"))
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_spending_fees")
                .build()
                .expect("Couldn't build entry"),
            // Reverse the previous UTXO entries
            EntryInput::builder()
                .entry_type("'UTXO_DROPPED_UTX_IN_PEN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build UTXO_DETECTED_PENDING_CR entry"),
            EntryInput::builder()
                .entry_type("'UTXO_DROPPED_UTX_IN_PEN_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_incoming_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = UtxoDroppedParams::defs();
        let template = NewTxTemplate::builder()
            .id(UTXO_DROPPED_ID)
            .code(UTXO_DROPPED_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build template");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
