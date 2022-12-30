use crate::{
    app::BlockchainConfig, batch_group::*, bdk::pg::Utxos, error::*, ledger::*, payout::*,
    primitives::*, wallet::*,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessBatchData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
}

#[instrument(name = "job.process_batch", skip(pool), err)]
pub async fn execute(
    pool: sqlx::PgPool,
    data: ProcessBatchData,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
) -> Result<ProcessBatchData, BriaError> {
    Ok(data)
}
