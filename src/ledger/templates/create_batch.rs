use bitcoin::Txid;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{
    tx_template::*, AccountId as LedgerAccountId, JournalId, SqlxLedger, SqlxLedgerError,
};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    error::*, ledger::constants::*, primitives::*, wallet::balance::WalletLedgerAccountIds,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBatchMeta {
    pub batch_id: BatchId,
    pub batch_group_id: BatchGroupId,
    pub bitcoin_tx_id: Txid,
}

#[derive(Debug)]
pub struct CreateBatchParams {
    pub journal_id: JournalId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub batch_satoshis: Satoshis,
    pub batch_true_fee_sats: Satoshis,
    pub correlation_id: Uuid,
    pub external_id: String,
    pub meta: CreateBatchMeta,
}

impl CreateBatchParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("ledger_account_outgoing_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("ledger_account_at_rest_id")
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
                .name("batch_true_fee")
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

impl From<CreateBatchParams> for TxParams {
    fn from(
        CreateBatchParams {
            journal_id,
            ledger_account_ids,
            batch_satoshis,
            batch_true_fee_sats,
            correlation_id,
            external_id,
            meta,
        }: CreateBatchParams,
    ) -> Self {
        let amount = batch_satoshis.to_btc();
        let batch_true_fee = batch_true_fee_sats.to_btc();
        let effective = Utc::now().date_naive();
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("ledger_account_outgoing_id", ledger_account_ids.outgoing_id);
        params.insert("ledger_account_at_rest_id", ledger_account_ids.at_rest_id);
        params.insert("ledger_account_fee_id", ledger_account_ids.fee_id);
        params.insert("amount", amount);
        params.insert("batch_true_fee", batch_true_fee);
        params.insert("correlation_id", correlation_id);
        params.insert("external_id", external_id);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct CreateBatch {}

impl CreateBatch {
    #[instrument(name = "ledger.create_batch.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), BriaError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .external_id("params.external_id")
            .correlation_id("params.correlation_id")
            .metadata("params.meta")
            .description("'Construct Batch'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            EntryInput::builder()
                .entry_type("'ENCUMBERED_WALLET_DR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_outgoing_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.amount")
                .build()
                .expect("Couldn't build ENCUMBERED_WALLET_DR entry"),
            EntryInput::builder()
                .entry_type("'ENCUMBERED_WALLET_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{}')", ONCHAIN_OUTGOING_ID))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.amount")
                .build()
                .expect("Couldn't build PENDING_ONCHAIN_CR entry"),
            EntryInput::builder()
                .entry_type("'PENDING_TX_OUTGOING_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{}')", ONCHAIN_OUTGOING_ID))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build PENDING_TX_OUTGOING_DR entry"),
            EntryInput::builder()
                .entry_type("'PENDING_TX_OUTGOING_CR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_outgoing_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build PENDING_TX_OUTGOING_CR entry"),
            EntryInput::builder()
                .entry_type("'SETTLED_TX_AT_REST_CR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_at_rest_id")
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build PENDING_TX_OUTGOING_DR entry"),
            EntryInput::builder()
                .entry_type("'SETTLED_TX_AT_REST_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{}')", ONCHAIN_OUTGOING_ID))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build PENDING_TX_OUTGOING_CR entry"),
            EntryInput::builder()
                .entry_type("'ENCUMBERED_BATCH_TRUE_FEE_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{}')", ONCHAIN_FEE_ID))
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.batch_true_fee")
                .build()
                .expect("Couldn't build PENDING_TX_OUTGOING_DR entry"),
            EntryInput::builder()
                .entry_type("'ENCUMBERED_BATCH_TRUE_FEE_CR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_fee_id")
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.batch_true_fee")
                .build()
                .expect("Couldn't build PENDING_TX_OUTGOING_CR entry"),
        ];

        let params = CreateBatchParams::defs();
        let template = NewTxTemplate::builder()
            .code(CREATE_BATCH_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build CREATE_BATCH_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
