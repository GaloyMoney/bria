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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoDroppedMeta {
    pub account_id: AccountId,
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: bitcoin::OutPoint,
    pub satoshis: Satoshis,
    pub address: bitcoin::Address,
    pub encumbered_spending_fees: EncumberedSpendingFees,
   pub confirmation_time: Option<BlockTime>,
}

#[derive(Debug)]
pub struct UtxoDroppedParams {
    pub journal_id: JournalId,
    pub onchain_incoming_account_id: LedgerAccountId,
    pub effective_incoming_account_id: LedgerAccountId,
    pub onchain_fee_account_id: LedgerAccountId,
    pub meta: UtxoDroppedMeta,
}

impl UtxoDroppedParams {
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
                .name("effective_incoming_account_id")
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

impl From<UtxoDroppedParams> for TxParams {
    fn from(
        UtxoDroppedParams {
            journal_id,
            onchain_incoming_account_id,
            effective_incoming_account_id,
            onchain_fee_account_id,
            meta,
        }: UtxoDroppedParams,
    ) -> Self {
        let amount = meta.satoshis.to_btc();
        let fees = meta
            .encumbered_spending_fees
            .values()
            .fold(Satoshis::ZERO, |s, v| s + *v)
            .to_btc();
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
        params.insert(
            "effective_incoming_account_id",
            effective_incoming_account_id,
        );
        params.insert("onchain_fee_account_id", onchain_fee_account_id);
        params.insert("amount", amount);
        params.insert("encumbered_spending_fees", fees);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

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
