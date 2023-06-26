mod legacy_batch_created;

use sqlx_ledger::{SqlxLedger, TransactionId, TxTemplateId};
use std::collections::HashMap;
use tracing::instrument;

use super::SpendSettledMeta;
use crate::ledger::{
    constants::{FIX_BATCH_CREATED_LEGACY_CODE, SPEND_SETTLED_ID},
    error::LedgerError,
};
use legacy_batch_created::*;

#[instrument(name = "ledger.fix_ledger_batch_created", skip_all)]
pub async fn legacy_batch_created(inner: &SqlxLedger) -> Result<(), LedgerError> {
    FixLegacyBatchCreated::init(inner).await?;

    let transactions = inner
        .transactions()
        .list_by_template_id(TxTemplateId::from(SPEND_SETTLED_ID))
        .await?;
    let mut txs = HashMap::new();
    let tx_ids: Vec<TransactionId> = transactions
        .into_iter()
        .filter_map(|tx| {
            tx.metadata::<SpendSettledMeta>()
                .transpose()
                .and_then(Result::ok)
                .and_then(|meta| {
                    meta.batch_info.map(|_| {
                        let id = tx.id;
                        txs.insert(tx.id, tx);
                        id
                    })
                })
        })
        .collect();
    let entries = inner.entries().list_by_transaction_ids(tx_ids).await?;
    for entry in entries.into_values().filter_map(|entries| {
        entries.into_iter().find(|e| {
            e.entry_type == "SPEND_SETTLED_CHG_SPENT_SET_DR"
                && e.units != rust_decimal::Decimal::ZERO
        })
    }) {
        inner
            .post_transaction(
                TransactionId::new(),
                FIX_BATCH_CREATED_LEGACY_CODE,
                Some(FixLegacyBatchCreatedParams {
                    tx: txs.remove(&entry.transaction_id).unwrap(),
                    account_id: entry.account_id,
                    units: entry.units,
                }),
            )
            .await?;
    }
    Ok(())
}
