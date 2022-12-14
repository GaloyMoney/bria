use serde::{Deserialize, Serialize};
use sqlx::Executor;
use tracing::instrument;

use crate::{batch_group::*, error::*, payout::*, primitives::*, wallet::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessBatchGroupData {
    pub batch_group_id: BatchGroupId,
    pub account_id: AccountId,
    batch_id: BatchId,
}

impl ProcessBatchGroupData {
    pub fn new(batch_group_id: BatchGroupId, account_id: AccountId) -> Self {
        Self {
            batch_group_id,
            account_id,
            batch_id: BatchId::new(),
        }
    }
}

#[instrument(
    name = "job.process_batch_group",
    skip(pool, payouts, batch_groups),
    err
)]
pub async fn execute(
    pool: sqlx::PgPool,
    payouts: Payouts,
    wallets: Wallets,
    batch_groups: BatchGroups,
    data: ProcessBatchGroupData,
) -> Result<ProcessBatchGroupData, BriaError> {
    let mut tx = pool.begin().await?;
    let unbatched_payouts = payouts.list_unbatched(&mut tx, data.batch_group_id).await?;
    let wallet_ids = unbatched_payouts.iter().map(|p| p.wallet_id).collect();
    let wallets = wallets.list_by_ids(wallet_ids).await?;

    // let mut old_keychain_inputs = HashMap::new();
    for (_, wallet) in wallets {
        for (keychain_id, keychain) in wallet.keychains[1..].iter() {
            //
        }
    }
    // find 'reserved' UTXOs
    // collect all "old" UTXOs
    //
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;

    Ok(data)
}
