use bdk::BlockTime;
use bitcoin::blockdata::transaction::{OutPoint, TxOut};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{
    tx_template::*, AccountId as LedgerAccountId, JournalId, SqlxLedger, SqlxLedgerError,
};
use tracing::instrument;
use uuid::Uuid;

use crate::{error::*, ledger::constants::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmedUtxoWithoutFeeReserveMeta {
    pub batch_id: BatchId,
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub txout: TxOut,
    pub confirmation_time: BlockTime,
}

#[derive(Debug)]
pub struct ConfirmedUtxoWithoutFeeReserveParams {
    pub journal_id: JournalId,
    pub incoming_ledger_account_id: LedgerAccountId,
    pub at_rest_ledger_account_id: LedgerAccountId,
    pub pending_id: Uuid,
    pub settled_id: Uuid,
    pub meta: ConfirmedUtxoWithoutFeeReserveMeta,
}

impl ConfirmedUtxoWithoutFeeReserveParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("ledger_account_incoming_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("ledger_account_at_rest_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("amount")
                .r#type(ParamDataType::DECIMAL)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("correlation_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("external_id")
                .r#type(ParamDataType::STRING)
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

impl From<ConfirmedUtxoWithoutFeeReserveParams> for TxParams {
    fn from(
        ConfirmedUtxoWithoutFeeReserveParams {
            journal_id,
            incoming_ledger_account_id,
            at_rest_ledger_account_id,
            pending_id,
            settled_id,
            meta,
        }: ConfirmedUtxoWithoutFeeReserveParams,
    ) -> Self {
        let amount = Satoshis::from(meta.txout.value).to_btc();
        let effective =
            NaiveDateTime::from_timestamp_opt(meta.confirmation_time.timestamp as i64, 0)
                .expect("Couldn't convert blocktime to NaiveDateTime")
                .date();
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("ledger_account_incoming_id", incoming_ledger_account_id);
        params.insert("ledger_account_at_rest_id", at_rest_ledger_account_id);
        params.insert("amount", amount);
        params.insert("external_id", settled_id.to_string());
        params.insert("correlation_id", pending_id);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct ConfirmedUtxoWithoutFeeReserve {}

impl ConfirmedUtxoWithoutFeeReserve {
    #[instrument(name = "ledger.confirmed_utxo_without_fee_reserve.init", skip_all)]
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
                .entry_type("'PENDING_ONCHAIN_DR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_incoming_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build PENDING_ONCHAIN_DR entry"),
            EntryInput::builder()
                .entry_type("'PENDING_ONCHAIN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build PENDING_ONCHAIN_CR entry"),
            EntryInput::builder()
                .entry_type("'SETTLED_ONCHAIN_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_INCOMING_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build SETTLED_ONCHAIN_DR entry"),
            EntryInput::builder()
                .entry_type("'SETTLED_ONCHAIN_CR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_at_rest_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build SETTLED_ONCHAIN_CR entry"),
        ];

        let params = ConfirmedUtxoWithoutFeeReserveParams::defs();
        let template = NewTxTemplate::builder()
            .code(CONFIRMED_UTXO_WITHOUT_FEE_RESERVE_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build CONFIRMED_UTXO_WITHOUT_FEE_RESERVE_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
