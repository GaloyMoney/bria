use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use electrum_client::{Client, ConfigBuilder};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{app::BlockchainConfig, batch::*, error::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchBroadcastingData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
}

#[instrument(name = "job.batch_broadcasting", skip(batches), err)]
pub async fn execute(
    data: BatchBroadcastingData,
    blockchain_cfg: BlockchainConfig,
    batches: Batches,
) -> Result<BatchBroadcastingData, BriaError> {
    let blockchain = init_electrum(&blockchain_cfg.electrum_url).await?;
    let batch = batches.find_by_id(data.account_id, data.batch_id).await?;
    if batch.accounting_complete() {
        if let Some(tx) = batch.signed_tx {
            blockchain.broadcast(&tx)?;
        }
    }
    Ok(data)
}

async fn init_electrum(electrum_url: &str) -> Result<ElectrumBlockchain, BriaError> {
    let blockchain = ElectrumBlockchain::from(Client::from_config(
        electrum_url,
        ConfigBuilder::new()
            .retry(10)
            .timeout(Some(4))
            .expect("couldn't set electrum timeout")
            .build(),
    )?);
    Ok(blockchain)
}
