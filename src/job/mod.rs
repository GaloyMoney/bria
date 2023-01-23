mod executor;
mod process_batch;
mod process_batch_group;
mod sync_wallet;

use sqlxmq::{job, CurrentJob, JobBuilder, JobRegistry, OwnedHandle};
use tracing::instrument;
use uuid::{uuid, Uuid};

use crate::{
    app::BlockchainConfig, batch::*, batch_group::*, error::*, ledger::Ledger, payout::*,
    primitives::*, wallet::*,
};
pub use executor::JobExecutionError;
use executor::JobExecutor;
use process_batch::ProcessBatchData;
use process_batch_group::ProcessBatchGroupData;

const SYNC_ALL_WALLETS_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
const PROCESS_ALL_BATCH_GROUPS_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");

#[derive(Debug, Clone)]
struct SyncAllWalletsDelay(std::time::Duration);
#[derive(Debug, Clone)]
struct ProcessAllBatchesDelay(std::time::Duration);

pub async fn start_job_runner(
    pool: &sqlx::PgPool,
    wallets: Wallets,
    batch_groups: BatchGroups,
    batches: Batches,
    payouts: Payouts,
    ledger: Ledger,
    sync_all_wallets_delay: std::time::Duration,
    process_all_batch_groups_delay: std::time::Duration,
    blockchain_cfg: BlockchainConfig,
) -> Result<OwnedHandle, BriaError> {
    let mut registry = JobRegistry::new(&[
        sync_all_wallets,
        sync_wallet,
        process_all_batch_groups,
        process_batch_group,
        process_batch,
    ]);
    registry.set_context(SyncAllWalletsDelay(sync_all_wallets_delay));
    registry.set_context(ProcessAllBatchesDelay(process_all_batch_groups_delay));
    registry.set_context(blockchain_cfg);
    registry.set_context(wallets);
    registry.set_context(batch_groups);
    registry.set_context(batches);
    registry.set_context(payouts);
    registry.set_context(ledger);

    Ok(registry.runner(pool).run().await?)
}

#[job(name = "sync_all_wallets", channel_name = "wallet_sync")]
async fn sync_all_wallets(
    mut current_job: CurrentJob,
    wallets: Wallets,
    SyncAllWalletsDelay(delay): SyncAllWalletsDelay,
) -> Result<(), BriaError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|_| async move {
            for id in wallets.all_ids().await? {
                let _ = spawn_sync_wallet(&pool, id).await;
            }
            Ok::<(), BriaError>(())
        })
        .await?;
    spawn_sync_all_wallets(current_job.pool(), delay).await?;
    Ok(())
}

#[job(name = "process_all_batch_groups", channel_name = "wallet_sync")]
async fn process_all_batch_groups(
    mut current_job: CurrentJob,
    batch_groups: BatchGroups,
    ProcessAllBatchesDelay(delay): ProcessAllBatchesDelay,
) -> Result<(), BriaError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|_| async move {
            for group in batch_groups.all().await? {
                if let Some(delay) = group.spawn_in() {
                    let _ = spawn_process_batch_group(
                        &pool,
                        ProcessBatchGroupData::new(group.id, group.account_id),
                        delay,
                    )
                    .await;
                }
            }
            Ok::<(), BriaError>(())
        })
        .await?;
    spawn_process_all_batch_groups(current_job.pool(), delay).await?;
    Ok(())
}

#[job(
    name = "sync_wallet",
    channel_name = "wallet_sync",
    retries = 20,
    ordered = true
)]
async fn sync_wallet(
    mut current_job: CurrentJob,
    wallets: Wallets,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
) -> Result<(), BriaError> {
    let wallet_id = WalletId::from(current_job.id());
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|_| async move {
            sync_wallet::execute(pool, wallets, wallet_id, blockchain_cfg, ledger).await
        })
        .await?;
    Ok(())
}

#[job(name = "process_batch_group", channel_name = "batch_group")]
async fn process_batch_group(
    mut current_job: CurrentJob,
    payouts: Payouts,
    wallets: Wallets,
    batch_groups: BatchGroups,
    batches: Batches,
) -> Result<(), BriaError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: ProcessBatchGroupData = data.expect("no ProcessBatchGroupData available");
            let (data, tx) =
                process_batch_group::execute(pool, payouts, wallets, batch_groups, batches, data)
                    .await?;
            if let Some(tx) = tx {
                spawn_process_batch(tx, ProcessBatchData::from(&data)).await?;
            }
            Ok::<_, BriaError>(data)
        })
        .await?;
    Ok(())
}

#[job(
    name = "process_batch",
    channel_name = "batch",
    retries = 20,
    ordered = true
)]
async fn process_batch(
    mut current_job: CurrentJob,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
    wallets: Wallets,
    batches: Batches,
) -> Result<(), BriaError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: ProcessBatchData = data.expect("no ProcessBatchData available");
            process_batch::execute(pool, data, blockchain_cfg, ledger, wallets, batches).await
        })
        .await?;
    Ok(())
}

#[instrument(skip_all, fields(error, error.level, error.message), err)]
pub async fn spawn_sync_all_wallets(
    pool: &sqlx::PgPool,
    duration: std::time::Duration,
) -> Result<(), BriaError> {
    match JobBuilder::new_with_id(SYNC_ALL_WALLETS_ID, "sync_all_wallets")
        .set_channel_name("wallet_sync")
        .set_delay(duration)
        .spawn(pool)
        .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(()),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(skip_all, fields(error, error.level, error.message), err)]
async fn spawn_sync_wallet(pool: &sqlx::PgPool, id: WalletId) -> Result<(), BriaError> {
    match sync_wallet
        .builder()
        .set_channel_args(&id.to_string())
        .spawn(pool)
        .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(()),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(skip_all, fields(error, error.level, error.message), err)]
pub async fn spawn_process_all_batch_groups(
    pool: &sqlx::PgPool,
    delay: std::time::Duration,
) -> Result<(), BriaError> {
    match JobBuilder::new_with_id(PROCESS_ALL_BATCH_GROUPS_ID, "process_all_batch_groups")
        .set_channel_name("batch_group")
        .set_delay(delay)
        .spawn(pool)
        .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(()),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(skip_all, fields(error, error.level, error.message), err)]
async fn spawn_process_batch_group(
    pool: &sqlx::PgPool,
    data: ProcessBatchGroupData,
    delay: std::time::Duration,
) -> Result<(), BriaError> {
    match JobBuilder::new_with_id(Uuid::from(data.batch_group_id), "process_batch_group")
        .set_delay(delay)
        .set_channel_name("batch_group")
        .set_channel_args(&data.account_id.to_string())
        .set_ordered(true)
        .set_json(&data)
        .expect("Couldn't set json")
        .spawn(pool)
        .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(()),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(skip_all, fields(error, error.level, error.message), err)]
async fn spawn_process_batch(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    data: ProcessBatchData,
) -> Result<(), BriaError> {
    match process_batch
        .builder()
        .set_json(&data)
        .expect("Couldn't set json")
        .set_channel_args(&data.batch_id.to_string())
        .spawn(&mut tx)
        .await
    {
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => {
            tx.commit().await?;
            Ok(())
        }
    }
}

impl From<&ProcessBatchGroupData> for ProcessBatchData {
    fn from(data: &ProcessBatchGroupData) -> Self {
        Self {
            account_id: data.account_id,
            batch_id: data.batch_id,
        }
    }
}
