use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use electrum_client::{Client, ConfigBuilder};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use std::collections::HashMap;

use super::error::JobError;
use crate::{app::BlockchainConfig, batch::*, bdk::error::BdkError, primitives::*};

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
    blockchain_cfg: BlockchainConfig,
    batches: Batches,
) -> Result<BatchBroadcastingData, JobError> {
    let blockchain = init_electrum(&blockchain_cfg.electrum_url).await?;
    let batch = batches.find_by_id(data.account_id, data.batch_id).await?;
    let span = tracing::Span::current();
    span.record("txid", tracing::field::display(batch.bitcoin_tx_id));
    if let Some(tx) = batch.get_tx_to_broadcast() {
        blockchain.broadcast(&tx).map_err(BdkError::BdkLibError)?;
        span.record("broadcast", true);
    }
    Ok(data)
}

async fn init_electrum(electrum_url: &str) -> Result<ElectrumBlockchain, BdkError> {
    let blockchain = ElectrumBlockchain::from(Client::from_config(
        electrum_url,
        ConfigBuilder::new().retry(10).timeout(Some(60)).build(),
    )?);
    Ok(blockchain)
}
