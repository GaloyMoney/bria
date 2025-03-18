use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use super::shared_meta::*;
use crate::{
    ledger::{constants::*, error::LedgerError, WalletLedgerAccountIds},
    primitives::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDroppedMeta {
    pub batch_info: BatchWalletInfo,
    pub tx_summary: WalletTransactionSummary,
    pub created_txn_id: LedgerTransactionId,
}

#[derive(Debug)]
pub struct BatchDroppedParams {
    pub journal_id: JournalId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub encumbered_fees: Satoshis,
    pub meta: BatchDroppedMeta,
}

impl BatchDroppedParams {
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
                .name("fees")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("change")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("encumbered_fees")
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

impl From<BatchDroppedParams> for TxParams {
    fn from(
        BatchDroppedParams {
            journal_id,
            ledger_account_ids,
            encumbered_fees,
            meta,
        }: BatchDroppedParams,
    ) -> Self {
        let WalletTransactionSummary {
            fee_sats,
            ref change_utxos,
            total_utxo_in_sats,
            total_utxo_settled_in_sats,
            ..
        } = meta.tx_summary;
        let total_utxo_in = total_utxo_in_sats.to_btc();
        let change = change_utxos
            .iter()
            .fold(Satoshis::ZERO, |s, u| s + u.satoshis)
            .to_btc();
        let fee_sats = fee_sats.to_btc();
        let encumbered_fees = encumbered_fees.to_btc();
        let effective = Utc::now().date_naive();
        let correlation_id = meta.created_txn_id;
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
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
            "onchain_outgoing_account_id",
            ledger_account_ids.onchain_outgoing_id,
        );
        params.insert(
            "onchain_income_account_id",
            ledger_account_ids.onchain_incoming_id,
        );
        params.insert(
            "onchain_at_rest_account_id",
            ledger_account_ids.onchain_at_rest_id,
        );
        params.insert("total_utxo_in", total_utxo_in);
        params.insert("total_utxo_settled_in", total_utxo_settled_in_sats.to_btc());
        params.insert("change", change);
        params.insert("fees", fee_sats);
        params.insert("encumbered_fees", encumbered_fees);
        params.insert("correlation_id", correlation_id);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct BatchDropped {}

impl BatchDropped {
    #[instrument(name = "ledger.batch_dropped.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<bool, LedgerError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .correlation_id("params.correlation_id")
            .metadata("params.meta")
            .description("'Drop Batch'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            // Reverse EFFECTIVE LOG_OUT_ENC entries
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_LOG_OUT_ENC_CR'")
                .currency("'BTC'")
                .account_id("params.effective_outgoing_account_id")
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_LOG_OUT_ENC_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_OUTGOING_ID}')"))
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            // Reverse EFFECTIVE LOG_OUT_PEN entries
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_LOG_OUT_PEN_DR'")
                .currency("'BTC'")
                .account_id("params.effective_outgoing_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_LOG_OUT_PEN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_OUTGOING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.change - params.fees")
                .build()
                .expect("Couldn't build entry"),
            // Reverse EFFECTIVE LOG_SET entries
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_LOG_SET_CR'")
                .currency("'BTC'")
                .account_id("params.effective_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.total_utxo_in - params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_LOG_SET_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_AT_REST_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.total_utxo_in - params.change")
                .build()
                .expect("Couldn't build entry"),
            // Reverse FEES entries
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_FEE_PEN_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_fee_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_FEE_PEN_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_FEE_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_FR_ENC_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_fee_account_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_FR_ENC_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_FEE_ID}')"))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_fees")
                .build()
                .expect("Couldn't build entry"),
            // Reverse UTXO entries
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_UTX_OUT_PEN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_OUTGOING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_UTX_OUT_PEN_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_outgoing_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.total_utxo_in - params.fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_UTX_SET_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.total_utxo_settled_in")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_UTX_SET_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.total_utxo_settled_in")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_CHG_ENC_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'BATCH_DROPPED_CHG_ENC_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_income_account_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.change")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = BatchDroppedParams::defs();
        let template = NewTxTemplate::builder()
            .id(BATCH_DROPPED_ID)
            .code(BATCH_DROPPED_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build BATCH_DROPPED_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(false),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(true),
        }
    }
}
