use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;
use uuid::Uuid;

use std::collections::HashMap;

use super::shared_meta::*;
use crate::{
    ledger::{constants::*, error::LedgerError, WalletLedgerAccountIds},
    primitives::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchBroadcastMeta {
    pub batch_info: BatchWalletInfo,
    pub encumbered_spending_fees: EncumberedSpendingFees,
    pub tx_summary: WalletTransactionSummary,
    pub withdraw_from_effective_when_settled: HashMap<bitcoin::OutPoint, Satoshis>,
}

#[derive(Debug)]
pub struct BatchBroadcastParams {
    pub journal_id: JournalId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub meta: BatchBroadcastMeta,
}

impl BatchBroadcastParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_fee_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_income_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("encumbered_spending_fees")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("change")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("correlation_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("meta")
                .r#type(ParamDataType::JSON)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("effective")
                .r#type(ParamDataType::DATE)
                .build()
                .unwrap(),
        ]
    }
}

impl From<BatchBroadcastParams> for TxParams {
    fn from(
        BatchBroadcastParams {
            journal_id,
            ledger_account_ids,
            meta,
        }: BatchBroadcastParams,
    ) -> Self {
        let effective = Utc::now().date_naive();
        let change = meta
            .tx_summary
            .change_utxos
            .iter()
            .fold(Satoshis::ZERO, |s, v| s + v.satoshis)
            .to_btc();
        let fees = meta
            .encumbered_spending_fees
            .values()
            .fold(Satoshis::ZERO, |s, v| s + *v)
            .to_btc();
        let batch_id = Uuid::from(meta.batch_info.batch_id);
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert(
            "onchain_income_account_id",
            ledger_account_ids.onchain_incoming_id,
        );
        params.insert("onchain_fee_account_id", ledger_account_ids.fee_id);
        params.insert("change", change);
        params.insert("encumbered_spending_fees", fees);
        params.insert("correlation_id", batch_id);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct BatchBroadcast {}

impl BatchBroadcast {
    #[instrument(name = "ledger.batch_broadcast.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), LedgerError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .metadata("params.meta")
            .description("'Submit Batch'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            // FEES
            EntryInput::builder()
                .entry_type("'BATCH_BROADCAST_FR_ENC_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_fee_account_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_spending_fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_BROADCAST_FR_ENC_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_FEE_ID}')"))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_spending_fees")
                .build()
                .expect("Couldn't build entry"),
            // UTXO
            EntryInput::builder()
                .entry_type("'BATCH_BROADCAST_CHG_ENC_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_income_account_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_BROADCAST_CHG_ENC_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_BROADCAST_CHG_PEN_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_BROADCAST_CHG_PEN_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_income_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = BatchBroadcastParams::defs();
        let template = NewTxTemplate::builder()
            .id(BATCH_BROADCAST_ID)
            .code(BATCH_BROADCAST_CODE)
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
