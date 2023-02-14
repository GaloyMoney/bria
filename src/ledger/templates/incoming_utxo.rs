use bdk::BlockTime;
use bitcoin::blockdata::transaction::{OutPoint, TxOut};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use crate::{error::*, ledger::constants::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingUtxoMeta {
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub txout: TxOut,
    pub confirmation_time: Option<BlockTime>,
}

#[derive(Debug)]
pub struct IncomingUtxoParams {
    pub journal_id: JournalId,
    pub ledger_account_incoming_id: LedgerAccountId,
    pub meta: IncomingUtxoMeta,
}

impl IncomingUtxoParams {
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
                .name("amount")
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

impl From<IncomingUtxoParams> for TxParams {
    fn from(
        IncomingUtxoParams {
            journal_id,
            ledger_account_incoming_id,
            meta,
        }: IncomingUtxoParams,
    ) -> Self {
        let amount = Satoshis::from(meta.txout.value).to_btc();
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
        params.insert("ledger_account_incoming_id", ledger_account_incoming_id);
        params.insert("amount", amount);
        params.insert("meta", meta);
        params.insert("effective", effective);
        params
    }
}

pub struct IncomingUtxo {}

impl IncomingUtxo {
    #[instrument(name = "ledger.incoming_utxo.init", skip_all)]
    pub async fn init(ledger: &SqlxLedger) -> Result<(), BriaError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .metadata("params.meta")
            .description("'Onchain tx in mempool'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            EntryInput::builder()
                .entry_type("'INCOMING_UTXO_PENDING_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_INCOMING_ID}')"))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build INCOMING_UTXO_PENDING_DR entry"),
            EntryInput::builder()
                .entry_type("'INCOMING_UTXO_PENDING_CR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_incoming_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build INCOMING_UTXO_PENDING_CR entry"),
        ];

        let params = IncomingUtxoParams::defs();
        let template = NewTxTemplate::builder()
            .id(INCOMING_UTXO_ID)
            .code(INCOMING_UTXO_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build INCOMING_UTXO_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
