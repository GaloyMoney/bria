use bdk::blockchain::Blockchain;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use std::collections::HashMap;

use super::{electrum::SharedElectrum, error::JobError};
use crate::{batch::*, bdk::error::BdkError, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchBroadcastingData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
    #[serde(flatten)]
    pub(super) tracing_data: HashMap<String, String>,
}

#[instrument(
    name = "job.batch_broadcasting",
    skip(batches),
    fields(txid, broadcast = false),
    err
)]
pub async fn execute(
    data: BatchBroadcastingData,
    electrum: SharedElectrum,
    batches: Batches,
) -> Result<BatchBroadcastingData, JobError> {
    let blockchain = electrum.get()?;
    let batch = batches.find_by_id(data.account_id, data.batch_id).await?;
    let span = tracing::Span::current();
    span.record("txid", tracing::field::display(batch.bitcoin_tx_id));
    if batch.accounting_complete() {
        if let Some(tx) = batch.signed_tx {
            blockchain.broadcast(&tx).map_err(BdkError::BdkLibError)?;
            span.record("broadcast", true);
        }
    }
    Ok(data)
}
