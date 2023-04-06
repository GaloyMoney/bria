use sqlx_ledger::{tx_template::*, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use super::confirmed_utxo::ConfirmedUtxoParams;
use crate::{error::*, ledger::constants::*};

pub struct ConfirmSpentUtxo {}

impl ConfirmSpentUtxo {
    #[instrument(name = "ledger.confirm_spent_utxo.init", skip_all)]
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
            // LOGICAL
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_LOGICAL_PENDING_DR'")
                .currency("'BTC'")
                .account_id("params.logical_incoming_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_LOGICAL_PENDING_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{LOGICAL_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_LOGICAL_SETTLED_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{LOGICAL_INCOMING_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_LOGICAL_SETTLED_CR'")
                .currency("'BTC'")
                .account_id("params.logical_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_LOGICAL_SETTLED_DR'")
                .currency("'BTC'")
                .account_id("params.logical_at_rest_account_id")
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.withdraw_from_logical_settled")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_LOGICAL_SETTLED_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{LOGICAL_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.withdraw_from_logical_settled")
                .build()
                .expect("Couldn't build entry"),
            // UTXO
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_UTXO_PENDING_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_incoming_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_UTXO_PENDING_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_UTXO_SETTLED_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_UTXO_SETTLED_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_UTXO_SETTLED_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'CONFIRM_SPENT_UTXO_UTXO_SETTLED_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
        ];

        let mut params = ConfirmedUtxoParams::defs();
        params.push(
            ParamDefinition::builder()
                .name("withdraw_from_logical_settled")
                .r#type(ParamDataType::DECIMAL)
                .default_expr("dec('0')")
                .build()
                .unwrap(),
        );
        let template = NewTxTemplate::builder()
            .id(CONFIRM_SPENT_UTXO_ID)
            .code(CONFIRM_SPENT_UTXO_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build CONFIRM_SPENT_UTXO_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
