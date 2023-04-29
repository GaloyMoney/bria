use bdk::BlockTime;
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use crate::{error::*, ledger::constants::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoDetectedMeta {
    pub account_id: AccountId,
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: bitcoin::OutPoint,
    pub satoshis: Satoshis,
    pub address: bitcoin::Address,
    pub encumbered_spending_fee_sats: Satoshis,
    pub confirmation_time: Option<BlockTime>,
}

#[derive(Debug)]
pub struct UtxoDetectedParams {
    pub journal_id: JournalId,
    pub onchain_incoming_account_id: LedgerAccountId,
    pub logical_incoming_account_id: LedgerAccountId,
    pub onchain_fee_account_id: LedgerAccountId,
    pub meta: UtxoDetectedMeta,
}

impl UtxoDetectedParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_incoming_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("logical_incoming_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_fee_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("amount")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("encumbered_spending_fees")
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

impl From<UtxoDetectedParams> for TxParams {
    fn from(
        UtxoDetectedParams {
            journal_id,
            onchain_incoming_account_id,
            logical_incoming_account_id,
            onchain_fee_account_id,
            meta,
        }: UtxoDetectedParams,
    ) -> Self {
        let amount = meta.satoshis.to_btc();
        let fees = meta.encumbered_spending_fee_sats.to_btc();
        let effective = meta
            .confirmation_time
            .as_ref()
            .map(|t| {
                NaiveDateTime::from_timestamp_opt(t.timestamp as i64, 0)
                    .expect("Couldn't convert blocktime to NaiveDateTime")
                    .date()
            })
            .unwrap_or_else(|| Utc::now().date_naive());
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("onchain_incoming_account_id", onchain_incoming_account_id);
        params.insert("logical_incoming_account_id", logical_incoming_account_id);
        params.insert("onchain_fee_account_id", onchain_fee_account_id);
        params.insert("amount", amount);
        params.insert("encumbered_spending_fees", fees);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct UtxoDetected {}

impl UtxoDetected {
    #[instrument(name = "ledger.utxo_detected.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), BriaError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .metadata("params.meta")
            .description("'Onchain tx in mempool'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            // LOGICAL
            EntryInput::builder()
                .entry_type("'UTXO_DETECTED_LOGICAL_PENDING_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{LOGICAL_INCOMING_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_DETECTED_LOGICAL_PENDING_CR'")
                .currency("'BTC'")
                .account_id("params.logical_incoming_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            // FEE
            EntryInput::builder()
                .entry_type("'UTXO_DETECTED_FEE_ENCUMEBERED_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_fee_account_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_spending_fees")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_DETECTED_FEE_ENCUMBERED_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_FEE_ID}')"))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.encumbered_spending_fees")
                .build()
                .expect("Couldn't build entry"),
            // UTXO
            EntryInput::builder()
                .entry_type("'UTXO_DETECTED_UTXO_PENDING_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build UTXO_DETECTED_PENDING_DR entry"),
            EntryInput::builder()
                .entry_type("'UTXO_DETECTED_UTXO_PENDING_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_incoming_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = UtxoDetectedParams::defs();
        let template = NewTxTemplate::builder()
            .id(UTXO_DETECTED_ID)
            .code(UTXO_DETECTED_CODE)
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
