use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::{app::BlockchainConfig, batch::*, error::*, ledger::*, primitives::*, wallet::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchWalletSigningData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
}

#[instrument(
    name = "job.batch_wallet_signing",
    skip(_pool, wallets, batches, ledger),
    err
)]
pub async fn execute(
    _pool: sqlx::PgPool,
    data: BatchWalletSigningData,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
    wallets: Wallets,
    batches: Batches,
) -> Result<BatchWalletAccountingData, BriaError> {
    // load and sign psbt
    Ok(data)
}
