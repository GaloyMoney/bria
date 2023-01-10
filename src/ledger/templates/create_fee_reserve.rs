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
pub struct CreateFeeReserveMeta {
    pub wallet_id: WalletId,
}

#[derive(Debug)]
pub struct CreateFeeReserveParams {
    pub journal_id: JournalId,
    pub ledger_account_fee_id: LedgerAccountId,
    pub pending_id: Uuid,
    pub satoshis: u64,
    pub meta: CreateFeeReserveMeta,
}

impl CreateFeeReserveParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("ledger_account_fee_id")
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

impl From<CreateFeeReserveParams> for TxParams {
    fn from(
        CreateFeeReserveParams {
            journal_id,
            ledger_account_fee_id,
            pending_id,
            satoshis,
            meta,
        }: CreateFeeReserveParams,
    ) -> Self {
        let effective = Utc::now().date_naive();
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("ledger_account_fee_id", ledger_account_fee_id);
        params.insert("amount", Decimal::from(satoshis) / SATS_PER_BTC);
        params.insert("external_id", pending_id.to_string());
        params.insert("correlation_id", pending_id);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct CreateFeeReserve {}

impl CreateFeeReserve {
    #[instrument(name = "ledger.create_fee_reserve.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), BriaError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .external_id("params.external_id")
            .correlation_id("params.correlation_id")
            .metadata("params.meta")
            .description("'Create Fee Reserve'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            EntryInput::builder()
                .entry_type("'ENCUMBERED_FEE_RESERVE_DR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_fee_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.amount")
                .build()
                .expect("Couldn't build ENCUMBERED_FEE_RESERVE_DR entry"),
            EntryInput::builder()
                .entry_type("'ENCUMBERED_FEE_RESERVE_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{}')", ONCHAIN_OUTGOING_ID))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.amount")
                .build()
                .expect("Couldn't build ENCUMBERED_FEE_RESERVE_CR entry"),
        ];

        let params = CreateFeeReserveParams::defs();
        let template = NewTxTemplate::builder()
            .code(CREATE_FEE_RESERVE_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build CREATE_FEE_RESERVE template");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
