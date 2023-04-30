use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use crate::{error::*, ledger::constants::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutQueuedMeta {
    pub account_id: AccountId,
    pub payout_id: PayoutId,
    pub wallet_id: WalletId,
    pub batch_group_id: BatchGroupId,
    pub destination: PayoutDestination,
}

#[derive(Debug)]
pub struct PayoutQueuedParams {
    pub journal_id: JournalId,
    pub logical_outgoing_account_id: LedgerAccountId,
    pub external_id: String,
    pub payout_satoshis: Satoshis,
    pub meta: PayoutQueuedMeta,
}

impl PayoutQueuedParams {
    pub fn defs() -> Vec<ParamDefinition> {
        vec![
            ParamDefinition::builder()
                .name("journal_id")
                .r#type(ParamDataType::UUID)
                .build()
                .unwrap(),
            ParamDefinition::builder()
                .name("logical_outgoing_account_id")
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

impl From<PayoutQueuedParams> for TxParams {
    fn from(
        PayoutQueuedParams {
            journal_id,
            logical_outgoing_account_id,
            external_id,
            payout_satoshis,
            meta,
        }: PayoutQueuedParams,
    ) -> Self {
        let effective = Utc::now().date_naive();
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("logical_outgoing_account_id", logical_outgoing_account_id);
        params.insert("amount", payout_satoshis.to_btc());
        params.insert("external_id", external_id);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct PayoutQueued {}

impl PayoutQueued {
    #[instrument(name = "ledger.payout_queued.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), BriaError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .external_id("params.external_id")
            .metadata("params.meta")
            .description("'Enqueueq payout'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            // LOGICAL
            EntryInput::builder()
                .entry_type("'PAYOUT_QUEUED_LOGICAL_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{LOGICAL_OUTGOING_ID}')"))
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'PAYOUT_QUEUED_LOGICAL_CR'")
                .currency("'BTC'")
                .account_id("params.logical_outgoing_account_id")
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = PayoutQueuedParams::defs();
        let template = NewTxTemplate::builder()
            .id(PAYOUT_QUEUED_ID)
            .code(PAYOUT_QUEUED_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build PAYOUT_QUEUED_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
