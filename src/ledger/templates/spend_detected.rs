use bdk::BlockTime;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use std::collections::HashMap;

use super::shared_meta::*;
use crate::{
    ledger::{constants::*, error::LedgerError, WalletLedgerAccountIds},
    primitives::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendDetectedMeta {
    pub encumbered_spending_fees: EncumberedSpendingFees,
    pub tx_summary: WalletTransactionSummary,
    pub withdraw_from_effective_when_settled: HashMap<bitcoin::OutPoint, Satoshis>,
    pub confirmation_time: Option<BlockTime>,
}

#[derive(Debug)]
pub struct SpendDetectedParams {
    pub journal_id: JournalId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub reserved_fees: Satoshis,
    pub meta: SpendDetectedMeta,
}

impl SpendDetectedParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("effective_outgoing_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("effective_at_rest_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_fee_account_id")
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
                .name("onchain_outgoing_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("encumbered_fee_credit")
                .default_expr("true")
                .r#type(ParamDataType::BOOLEAN)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("encumbered_fee_diff")
                .r#type(ParamDataType::DECIMAL)
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
                .name("total_utxo_settled_in")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("effective_settled_out")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("change")
                .r#type(ParamDataType::DECIMAL)
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

impl From<SpendDetectedParams> for TxParams {
    fn from(
        SpendDetectedParams {
            journal_id,
            ledger_account_ids,
            reserved_fees,
            meta,
        }: SpendDetectedParams,
    ) -> Self {
        let effective = meta
            .confirmation_time
            .as_ref()
            .map(|t| {
                DateTime::from_timestamp(t.timestamp as i64, 0)
                    .expect("Couldn't convert blocktime to NaiveDateTime")
                    .date_naive()
            })
            .unwrap_or_else(|| Utc::now().date_naive());
        let encumbered_fee_diff = reserved_fees
            - meta
                .encumbered_spending_fees
                .values()
                .fold(Satoshis::ZERO, |t, d| t + *d);
        let WalletTransactionSummary {
            total_utxo_in_sats,
            total_utxo_settled_in_sats,
            fee_sats,
            ref change_utxos,
            ..
        } = meta.tx_summary;
        let change_sats = change_utxos
            .iter()
            .fold(Satoshis::ZERO, |s, d| s + d.satoshis);
        let deferred_effective_settled = meta
            .withdraw_from_effective_when_settled
            .values()
            .fold(Satoshis::ZERO, |t, d| t + *d);
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("meta", meta);
        params.insert(
            "effective_outgoing_account_id",
            ledger_account_ids.effective_outgoing_id,
        );
        params.insert(
            "effective_at_rest_account_id",
            ledger_account_ids.effective_at_rest_id,
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
        if encumbered_fee_diff < Satoshis::ZERO {
            params.insert("encumbered_fee_credit", false);
        };
        params.insert("encumbered_fee_diff", encumbered_fee_diff.abs().to_btc());
        params.insert("fees", fee_sats.to_btc());
        params.insert("total_utxo_in", total_utxo_in_sats.to_btc());
        params.insert("total_utxo_settled_in", total_utxo_settled_in_sats.to_btc());
        params.insert(
            "effective_settled_out",
            (total_utxo_in_sats - change_sats - deferred_effective_settled).to_btc(),
        );
        params.insert("change", change_sats.to_btc());
        params.insert("effective", effective);
        params
    }
}

pub struct SpendDetected {}

impl SpendDetected {
    #[instrument(name = "ledger.spend_detected.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), LedgerError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .metadata("params.meta")
            .description("'External Spend'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            // EFFECTIVE
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_LOG_OUT_PEN_CR'")
                .currency("'BTC'")
                .account_id("params.effective_outgoing_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_LOG_OUT_PEN_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_OUTGOING_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_LOG_SET_DR'")
                .currency("'BTC'")
                .account_id("params.effective_at_rest_account_id")
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.effective_settled_out")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_LOG_SET_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_AT_REST_ID}')"))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.effective_settled_out")
                .build()
                .expect("Couldn't build entry"),
            // FEES
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_FEE_PEN_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_fee_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_FEE_PEN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_FEE_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_FR_ENC_DR'")
                .currency("'BTC'")
                .account_id(
                    format!("params.encumbered_fee_credit ? uuid('{ONCHAIN_FEE_ID}') : params.onchain_fee_account_id"),
                )
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_fee_diff")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_FR_ENC_CR'")
                .currency("'BTC'")
                .account_id(
                    format!("params.encumbered_fee_credit ? params.onchain_fee_account_id : uuid('{ONCHAIN_FEE_ID}')"),
                )
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_fee_diff")
                .build()
                .expect("Couldn't build entry"),
            // UTXO
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_UTX_OUT_PEN_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_OUTGOING_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_UTX_OUT_PEN_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_outgoing_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_UTX_SET_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.total_utxo_settled_in")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_UTX_SET_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.total_utxo_settled_in")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_CHG_PEN_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPEND_DETECTED_CHG_PEN_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_income_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = SpendDetectedParams::defs();
        let template = NewTxTemplate::builder()
            .id(SPEND_DETECTED_ID)
            .code(SPEND_DETECTED_CODE)
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
