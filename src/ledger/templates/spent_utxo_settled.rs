use sqlx_ledger::{tx_template::*, SqlxLedger, SqlxLedgerError};
use tracing::instrument;

use super::utxo_settled::UtxoSettledParams;
use crate::{error::*, ledger::constants::*};

pub struct SpentUtxoSettled {}

impl SpentUtxoSettled {
    #[instrument(name = "ledger.spent_utxo_settled.init", skip_all)]
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
            // EFFECTIVE
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_LOG_IN_PEN_DR'")
                .currency("'BTC'")
                .account_id("params.effective_incoming_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_LOG_IN_PEN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_LOG_SET_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_AT_REST_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_LOG_SET_CR'")
                .currency("'BTC'")
                .account_id("params.effective_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_LOG_SET_DR'")
                .currency("'BTC'")
                .account_id("params.effective_at_rest_account_id")
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.withdraw_from_effective_settled")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_LOG_SET_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{EFFECTIVE_AT_REST_ID}')"))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.withdraw_from_effective_settled")
                .build()
                .expect("Couldn't build entry"),
            // UTXO
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_UTX_IN_PEN_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_incoming_account_id")
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_UTX_IN_PEN_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_INCOMING_ID}')"))
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_UTX_SET_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_UTX_SET_CR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_UTX_SET_DR'")
                .currency("'BTC'")
                .account_id("params.onchain_at_rest_account_id")
                .direction("DEBIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
            EntryInput::builder()
                .entry_type("'SPENT_UTXO_SETTLED_UTX_SET_CR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{ONCHAIN_UTXO_AT_REST_ID}')"))
                .direction("CREDIT")
                .layer("SETTLED")
                .units("params.amount")
                .build()
                .expect("Couldn't build entry"),
        ];

        let mut params = UtxoSettledParams::defs();
        params.push(
            ParamDefinition::builder()
                .name("withdraw_from_effective_settled")
                .r#type(ParamDataType::DECIMAL)
                .default_expr("decimal('0')")
                .build()
                .unwrap(),
        );
        let template = NewTxTemplate::builder()
            .id(SPENT_UTXO_SETTLED_ID)
            .code(SPENT_UTXO_SETTLED_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build SPENT_UTXO_SETTLED_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
