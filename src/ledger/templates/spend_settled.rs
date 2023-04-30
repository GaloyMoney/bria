use bdk::BlockTime;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use super::shared_meta::*;
use crate::{
    error::*, ledger::constants::*, primitives::*, wallet::balance::WalletLedgerAccountIds,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendSettledMeta {
    pub batch_info: Option<BatchWalletInfo>,
    pub tx_summary: WalletTransactionSummary,
    pub confirmation_time: BlockTime,
}

#[derive(Debug)]
pub struct SpendSettledParams {
    pub journal_id: JournalId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub spend_detected_tx_id: LedgerTransactionId,
    pub change_spent: bool,
    pub meta: SpendSettledMeta,
}

impl SpendSettledParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("logical_outgoing_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_fee_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_outgoing_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_at_rest_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_income_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("fees")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("total_utxo_in")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("change")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("change_spent")
                .r#type(ParamDataType::BOOLEAN)
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

impl From<SpendSettledParams> for TxParams {
    fn from(
        SpendSettledParams {
            journal_id,
            ledger_account_ids,
            spend_detected_tx_id: pending_id,
            change_spent,
            meta,
        }: SpendSettledParams,
    ) -> Self {
        let effective =
            NaiveDateTime::from_timestamp_opt(meta.confirmation_time.timestamp as i64, 0)
                .expect("Couldn't convert blocktime to NaiveDateTime")
                .date();
        let WalletTransactionSummary {
            total_utxo_in_sats,
            change_sats,
            fee_sats,
            ..
        } = meta.tx_summary;
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("meta", meta);
        params.insert(
            "logical_outgoing_account_id",
            ledger_account_ids.logical_outgoing_id,
        );
        params.insert("onchain_fee_account_id", ledger_account_ids.fee_id);
        params.insert(
            "onchain_at_rest_account_id",
            ledger_account_ids.onchain_at_rest_id,
        );
        params.insert(
            "onchain_income_account_id",
            ledger_account_ids.onchain_incoming_id,
        );
        params.insert(
            "onchain_outgoing_account_id",
            ledger_account_ids.onchain_outgoing_id,
        );
        params.insert("fees", fee_sats.to_btc());
        params.insert("total_utxo_in", total_utxo_in_sats.to_btc());
        params.insert("change", change_sats.to_btc());
        params.insert("change_spent", change_spent);
        params.insert("correlation_id", pending_id);
        params.insert("effective", effective);
        params
    }
}

pub struct SpendSettled {}

impl SpendSettled {
    #[instrument(name = "ledger.spend_settled.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), BriaError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .correlation_id("params.correlation_id")
            .metadata("params.meta")
            .description("'Spend tx confirmed'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            // LOGICAL
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_LOGICAL_PENDING_DR'")
                .currency("'BTC'")
                .account_id("params.logical_outgoing_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_LOGICAL_PENDING_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{LOGICAL_OUTGOING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_LOGICAL_SETTLED_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{LOGICAL_OUTGOING_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_LOGICAL_SETTLED_CR'")
                .currency("'BTC'")
                .account_id("params.logical_outgoing_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            // FEES
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_FEE_PENDING_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_FEE_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_FEE_PENDING_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_fee_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_FEE_SETTLED_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_fee_account_id")
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_FEE_SETTLED_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_FEE_ID}')"))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.fees")
                .build()
                .expect("Couldn't build entry"),
            // UTXO
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_PENDING_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_outgoing_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_PENDING_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_OUTGOING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_SETTLED_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_OUTGOING_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.total_utxo_in - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_SETTLED_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_outgoing_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.total_utxo_in - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_PENDING_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_income_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_PENDING_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_SETTLED_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_SETTLED_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_SETTLED_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.change_spent ? params.change : 0")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_SETTLED_UTXO_SETTLED_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.change_spent ? params.change : 0")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = SpendSettledParams::defs();
        let template = NewTxTemplate::builder()
            .id(SPEND_SETTLED_ID)
            .code(SPEND_SETTLED_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build SPEND_SETTLED_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
