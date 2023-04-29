use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;
use uuid::Uuid;

use std::collections::HashMap;

use super::shared_meta::TransactionSummary;
use crate::{
    error::*, ledger::constants::*, primitives::*, wallet::balance::WalletLedgerAccountIds,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSubmittedMeta {
    pub batch_id: BatchId,
    pub batch_group_id: BatchGroupId,
    pub encumbered_spending_fee_sats: Option<Satoshis>,
    pub tx_summary: TransactionSummary,
    pub withdraw_from_logical_when_settled: HashMap<bitcoin::OutPoint, Satoshis>,
}

#[derive(Debug)]
pub struct BatchSubmittedParams {
    pub journal_id: JournalId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub meta: BatchSubmittedMeta,
}

impl BatchSubmittedParams {
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

impl From<BatchSubmittedParams> for TxParams {
    fn from(
        BatchSubmittedParams {
            journal_id,
            ledger_account_ids,
            meta,
        }: BatchSubmittedParams,
    ) -> Self {
        let effective = Utc::now().date_naive();
        let change = meta.tx_summary.change_sats.to_btc();
        let fees = meta
            .encumbered_spending_fee_sats
            .unwrap_or(Satoshis::ZERO)
            .to_btc();
        let batch_id = Uuid::from(meta.batch_id);
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

pub struct BatchSubmitted {}

impl BatchSubmitted {
    #[instrument(name = "ledger.batch_submitted.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), BriaError> {
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
                .entry_type("'BATCH_SUBMITTED_FEE_ENCUMBERED_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_fee_account_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_spending_fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_SUBMITTED_FEE_ENCUMBERED_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_FEE_ID}')"))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_spending_fees")
                .build()
                .expect("Couldn't build entry"),
            // UTXO
            EntryInput::builder()
                .entry_type("'BATCH_SUBMITTED_UTXO_ENCUMBERED_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_income_account_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_SUBMITTED_UTXO_ENCUMBERED_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_SUBMITTED_UTXO_PENDING_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_SUBMITTED_UTXO_PENDING_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_income_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = BatchSubmittedParams::defs();
        let template = NewTxTemplate::builder()
            .id(BATCH_SUBMITTED_ID)
            .code(BATCH_SUBMITTED_CODE)
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
