use serde::{Deserialize, Serialize};
use tracing::instrument;

use std::collections::HashMap;

use crate::{
    app::BlockchainConfig, batch::*, error::*, primitives::*, signing_session::*, wallet::*,
    xpub::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchWalletSigningData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
    pub(super) wallet_id: WalletId,
    #[serde(flatten)]
    pub(super) tracing_data: HashMap<String, String>,
}

#[instrument(
    name = "job.batch_wallet_signing",
    skip(pool, wallets, batches, xpubs),
    err
)]
pub async fn execute(
    pool: sqlx::PgPool,
    data: BatchWalletSigningData,
    blockchain_cfg: BlockchainConfig,
    batches: Batches,
    wallets: Wallets,
    xpubs: XPubs,
) -> Result<BatchWalletSigningData, BriaError> {
    // load and sign psbt
    Ok(data)
}
