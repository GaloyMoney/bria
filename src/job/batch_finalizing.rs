use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{app::BlockchainConfig, batch::*, error::*, ledger::*, primitives::*, wallet::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFinalizingData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
}

#[instrument(
    name = "job.batch_finalizing",
    skip(_pool, _wallets, _batches, _ledger),
    err
)]
pub async fn execute(
    _pool: sqlx::PgPool,
    data: BatchFinalizingData,
    blockchain_cfg: BlockchainConfig,
    _ledger: Ledger,
    _wallets: Wallets,
    _batches: Batches,
) -> Result<BatchFinalizingData, BriaError> {
    // load and sign psbt
    Ok(data)
}
