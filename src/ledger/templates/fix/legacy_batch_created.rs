use rust_decimal::Decimal;
use sqlx_ledger::{
    transaction::Transaction, tx_template::*, AccountId, SqlxLedger, SqlxLedgerError,
};
use tracing::instrument;

use crate::ledger::{
    constants::{
        FIX_BATCH_CREATED_LEGACY_CODE, FIX_BATCH_CREATED_LEGACY_ID, ONCHAIN_UTXO_AT_REST_ID,
    },
    LedgerError,
};

#[derive(Debug)]
pub struct FixLegacyBatchCreatedParams {
    pub(super) tx: Transaction,
    pub(super) account_id: AccountId,
    pub(super) units: Decimal,
}

impl FixLegacyBatchCreatedParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("onchain_at_rest_account_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("revert_amount")
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

impl From<FixLegacyBatchCreatedParams> for TxParams {
    fn from(
        FixLegacyBatchCreatedParams {
            tx,
            account_id,
            units,
        }: FixLegacyBatchCreatedParams,
    ) -> Self {
        let mut params = Self::default();
        params.insert("journal_id", tx.journal_id);
        params.insert("effective", tx.effective);
        params.insert(
            "meta",
            serde_json::to_value(tx.metadata_json).expect("Couldn't serialize"),
        );
        params.insert("correlation_id", tx.correlation_id);
        params.insert("onchain_at_rest_account_id", account_id);
        params.insert("revert_amount", units);
        params
    }
}

pub struct FixLegacyBatchCreated {}

impl FixLegacyBatchCreated {
    #[instrument(name = "ledger.fix_legacy_batch_created.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), LedgerError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .correlation_id("params.correlation_id")
            .metadata("params.meta")
            .description("'Fix legacy create batch'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            EntryInput::builder()
                .entry_type("'FIX_LEGACY_BATCH_CREATED_CHG_SPENT_SET_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.revert_amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'FIX_LEGACY_BATCH_CREATED_CHG_SPENT_SET_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.revert_amount")
                .build()
                .expect("Couldn't build entry"),
        ];
        let params = FixLegacyBatchCreatedParams::defs();
        let template = NewTxTemplate::builder()
            .id(FIX_BATCH_CREATED_LEGACY_ID)
            .code(FIX_BATCH_CREATED_LEGACY_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build FIX_LEGACY_BATCH_CREATED template");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
