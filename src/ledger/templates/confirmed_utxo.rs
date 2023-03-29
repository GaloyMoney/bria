use bdk::BlockTime;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{tx_template::*, JournalId, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use crate::{error::*, ledger::constants::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmedUtxoMeta {
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: bitcoin::OutPoint,
    pub satoshis: Satoshis,
    pub address: String,
    pub confirmation_time: BlockTime,
}

#[derive(Debug)]
pub struct ConfirmedUtxoParams {
    pub journal_id: JournalId,
    pub incoming_ledger_account_id: LedgerAccountId,
    pub at_rest_ledger_account_id: LedgerAccountId,
    pub fee_ledger_account_id: LedgerAccountId,
    pub spending_fee_satoshis: Satoshis,
    pub pending_id: LedgerTransactionId,
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

impl From<ConfirmedUtxoParams> for TxParams {
    fn from(
        ConfirmedUtxoParams {
            journal_id,
            incoming_ledger_account_id,
            at_rest_ledger_account_id,
            fee_ledger_account_id,
            spending_fee_satoshis: fees,
            pending_id,
            meta,
        }: ConfirmedUtxoParams,
    ) -> Self {
        let amount = meta.satoshis.to_btc();
        let effective =
            NaiveDateTime::from_timestamp_opt(meta.confirmation_time.timestamp as i64, 0)
                .expect("Couldn't convert blocktime to NaiveDateTime")
                .date();
        let meta = serde_json::to_value(meta).expect("Couldn't serialize meta");
        let mut params = Self::default();
        params.insert("journal_id", journal_id);
        params.insert("ledger_account_incoming_id", incoming_ledger_account_id);
        params.insert("ledger_account_at_rest_id", at_rest_ledger_account_id);
        params.insert("ledger_account_fee_id", fee_ledger_account_id);
        params.insert("fees", fees.to_btc());
        params.insert("amount", amount);
        params.insert("correlation_id", pending_id);
        params.insert("meta", meta);
        params.insert("effective", effective);
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
            .correlation_id("params.correlation_id")
            .metadata("params.meta")
            .description("'Onchain tx confirmed'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            EntryInput::builder()
                .entry_type("'CONFIRMED_UTXO_PENDING_DR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_incoming_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build CONFIRMED_UTXO_PENDING_DR entry"),
            EntryInput::builder()
                .entry_type("'CONFIRMED_UTXO_PENDING_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build PENDING_ONCHAIN_CR entry"),
            EntryInput::builder()
                .entry_type("'CONFIRMED_UTXO_SETTLED_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_INCOMING_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build CONFIRMED_UTXO_SETTLED_DR entry"),
            EntryInput::builder()
                .entry_type("'CONFIRMED_UTXO_SETTLED_CR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_at_rest_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build CONFIRMED_UTXO_SETTLED_CR entry"),
            EntryInput::builder()
                .entry_type("'CONFIRMED_UTXO_FR_DR'")
                .currency("'BTC'")
                .account_id("params.ledger_account_fee_id")
                .direction("DEBIT")
                .layer("ENCUMBERED")
                .units("params.fees")
                .build()
                .expect("Couldn't build CONFIRMED_UTXO_FR_DR entry"),
            EntryInput::builder()
                .entry_type("'CONFIRMED_UTXO_FR_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_OUTGOING_ID}')"))
                .direction("CREDIT")
                .layer("ENCUMBERED")
                .units("params.fees")
                .build()
                .expect("Couldn't build CONFIRMED_UTXO_FR_DR entry"),
        ];

        let params = ConfirmedUtxoParams::defs();
        let template = NewTxTemplate::builder()
            .id(CONFIRMED_UTXO_ID)
            .code(CONFIRMED_UTXO_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build CONFIRMED_UTXO_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
