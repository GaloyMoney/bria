use sqlx::Executor;
use tracing::instrument;

use crate::{batch_group::*, error::*, payout::*, primitives::*, wallet::*};

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
    id: BatchGroupId,
) -> Result<(), BriaError> {
    let mut tx = pool.begin().await?;
    tx.execute("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;")
        .await?;
    let unbatched_payouts = payouts.list_unbatched(&mut tx, id).await?;
    Ok(())
}
