use bdk::BlockTime;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use crate::{
    ledger::{constants::*, error::LedgerError, WalletLedgerAccountIds},
    primitives::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtxoSettledMeta {
    pub account_id: AccountId,
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: bitcoin::OutPoint,
    pub satoshis: Satoshis,
    pub address: Address,
    pub confirmation_time: BlockTime,
    pub already_spent_tx_id: Option<LedgerTransactionId>,
}

#[derive(Debug)]
pub struct UtxoSettledParams {
    pub journal_id: JournalId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub pending_id: LedgerTransactionId,
    pub meta: UtxoSettledMeta,
}

impl UtxoSettledParams {
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
                .name("onchain_at_rest_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("effective_incoming_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("effective_at_rest_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("amount")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("fees")
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

impl From<UtxoSettledParams> for TxParams {
    fn from(
        UtxoSettledParams {
            journal_id,
            ledger_account_ids: accounts,
            pending_id,
            meta,
        }: UtxoSettledParams,
    ) -> Self {
        let amount = meta.satoshis.to_btc();
        let effective =
            NaiveDateTime::from_timestamp_opt(meta.confirmation_time.timestamp as i64, 0)
                .expect("Couldn't convert blocktime to NaiveDateTime")
                .date();
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("onchain_incoming_account_id", accounts.onchain_incoming_id);
        params.insert("onchain_at_rest_account_id", accounts.onchain_at_rest_id);
        params.insert(
            "effective_incoming_account_id",
            accounts.effective_incoming_id,
        );
        params.insert(
            "effective_at_rest_account_id",
            accounts.effective_at_rest_id,
        );
        params.insert("amount", amount);
        params.insert("correlation_id", pending_id);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct UtxoSettled {}

impl UtxoSettled {
    #[instrument(name = "ledger.utxo_settled.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), LedgerError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .correlation_id("params.correlation_id")
            .metadata("params.meta")
            .description("'Onchain tx confirmed'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            // EFFECTIVE
            EntryInput::builder()
                .entry_type("'UTXO_SETTLED_LOG_IN_PEN_DR'")
                .currency("'BTC'")
                .account_id("params.effective_incoming_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_SETTLED_LOG_IN_PEN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_SETTLED_LOG_SET_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_AT_REST_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_SETTLED_LOG_SET_CR'")
                .currency("'BTC'")
                .account_id("params.effective_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            // UTXO
            EntryInput::builder()
                .entry_type("'UTXO_SETTLED_UTX_IN_PEN_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_incoming_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_SETTLED_UTX_IN_PEN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_SETTLED_UTX_SET_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'UTXO_SETTLED_UTX_SET_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = UtxoSettledParams::defs();
        let template = NewTxTemplate::builder()
            .id(UTXO_SETTLED_ID)
            .code(UTXO_SETTLED_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build UTXO_SETTLED_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
