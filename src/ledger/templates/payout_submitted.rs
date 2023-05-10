use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use crate::{error::*, ledger::constants::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutSubmittedMeta {
    pub account_id: AccountId,
    pub payout_id: PayoutId,
    pub wallet_id: WalletId,
    pub payout_queue_id: PayoutQueueId,
    pub profile_id: ProfileId,
    pub satoshis: Satoshis,
    pub destination: PayoutDestination,
}

#[derive(Debug)]
pub struct PayoutSubmittedParams {
    pub journal_id: JournalId,
    pub effective_outgoing_account_id: LedgerAccountId,
    pub external_id: String,
    pub meta: PayoutSubmittedMeta,
}

impl PayoutSubmittedParams {
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

impl From<PayoutSubmittedParams> for TxParams {
    fn from(
        PayoutSubmittedParams {
            journal_id,
            effective_outgoing_account_id,
            external_id,
            meta,
        }: PayoutSubmittedParams,
    ) -> Self {
        let effective = Utc::now().date_naive();
        let amount = meta.satoshis.to_btc();
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert(
            "effective_outgoing_account_id",
            effective_outgoing_account_id,
        );
        params.insert("amount", amount);
        params.insert("external_id", external_id);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct PayoutSubmitted {}

impl PayoutSubmitted {
    #[instrument(name = "ledger.payout_submitted.init", skip_all)]
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
            // EFFECTIVE
            EntryInput::builder()
                .entry_type("'PAYOUT_SUBMITTED_LOG_OUT_ENC_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_OUTGOING_ID}')"))
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'PAYOUT_SUBMITTED_LOG_OUT_ENC_CR'")
                .currency("'BTC'")
                .account_id("params.effective_outgoing_account_id")
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
        ];

        let params = PayoutSubmittedParams::defs();
        let template = NewTxTemplate::builder()
            .id(PAYOUT_SUBMITTED_ID)
            .code(PAYOUT_SUBMITTED_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build PAYOUT_SUBMITTED_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
