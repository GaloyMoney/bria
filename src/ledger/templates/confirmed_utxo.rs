use bitcoin::blockdata::transaction::{OutPoint, TxOut};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{
    tx_template::*, AccountId as LedgerAccountId, JournalId, SqlxLedger, SqlxLedgerError,
};
use tracing::instrument;
use uuid::Uuid;

use crate::{error::*, ledger::constants::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmedUtxoMeta {
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub txout: TxOut,
}

#[derive(Debug)]
pub struct ConfirmedUtxoParams {
    pub journal_id: JournalId,
    pub recipient_account_id: LedgerAccountId,
    pub pending_id: Uuid,
    pub meta: ConfirmedUtxoMeta,
}

impl ConfirmedUtxoParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("recipient_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("amount")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("external_id")
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

impl From<ConfirmedUtxoParams> for TxParams {
    fn from(
        ConfirmedUtxoParams {
            journal_id,
            recipient_account_id,
            pending_id,
            meta,
        }: ConfirmedUtxoParams,
    ) -> Self {
        let amount = Decimal::from(meta.txout.value) / SATS_PER_BTC;
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("recipient_account_id", recipient_account_id);
        params.insert("amount", amount);
        params.insert("external_id", pending_id);
        params.insert("meta", meta);
        params.insert("effective", Utc::now().date_naive());
        params
    }
}

pub struct ConfirmedUtxo {}

impl ConfirmedUtxo {
    #[instrument(name = "ledger.confirmed_utxo.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), BriaError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .external_id("params.external_id")
            .correlation_id("params.correlation_id")
            .metadata("params.meta")
            .description("'Onchain tx confirmed'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            EntryInput::builder()
                .entry_type("'PENDING_ONCHAIN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{}')", ONCHAIN_INCOMING_ID))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build SETTLED_ONCHAIN_CR entry"),
            EntryInput::builder()
                .entry_type("'SETTLED_ONCHAIN_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{}')", ONCHAIN_INCOMING_ID))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build SETTLED_ONCHAIN_DR entry"),
            EntryInput::builder()
                .entry_type("'PENDING_ONCHAIN_DR'")
                .currency("'BTC'")
                .account_id("params.recipient_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build PENDING_ONCHAIN_DR entry"),
            EntryInput::builder()
                .entry_type("'SETTLED_ONCHAIN_CR'")
                .currency("'BTC'")
                .account_id("params.recipient_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build SETTLED_ONCHAIN_CR entry"),
        ];

        let params = ConfirmedUtxoParams::defs();
        let template = NewTxTemplate::builder()
            .code(CONFIRMED_UTXO_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build PENDING_ONCHAIN_CREDIT_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
