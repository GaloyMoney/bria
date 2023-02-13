mod batch_wallet_accounting;
mod batch_wallet_finalizing;
mod batch_wallet_signing;
mod executor;
mod process_batch_group;
mod sync_wallet;

use sqlxmq::{job, CurrentJob, JobBuilder, JobRegistry, OwnedHandle};
use tracing::instrument;
use uuid::{uuid, Uuid};

use crate::{
    app::BlockchainConfig, batch::*, batch_group::*, error::*, ledger::Ledger, payout::*,
    primitives::*, wallet::*,
};
use batch_wallet_accounting::BatchWalletAccountingData;
use batch_wallet_finalizing::BatchWalletFinalizingData;
use batch_wallet_signing::BatchWalletSigningData;
pub use executor::JobExecutionError;
use executor::JobExecutor;
use process_batch_group::ProcessBatchGroupData;
use sync_wallet::SyncWalletData;

const SYNC_ALL_WALLETS_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
const PROCESS_ALL_BATCH_GROUPS_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");

#[derive(Debug, Clone)]
struct SyncAllWalletsDelay(std::time::Duration);
#[derive(Debug, Clone)]
struct ProcessAllBatchesDelay(std::time::Duration);

#[allow(clippy::too_many_arguments)]
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
        batch_wallet_accounting,
        batch_wallet_signing,
        batch_wallet_finalizing,
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
                let _ = spawn_sync_wallet(&pool, SyncWalletData::new(id)).await;
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
    batches: Batches,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
) -> Result<(), BriaError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: SyncWalletData = data.expect("no SyncWalletData available");
            sync_wallet::execute(pool, wallets, batches, blockchain_cfg, ledger, data).await
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
            let (data, res) =
                process_batch_group::execute(pool, payouts, wallets, batch_groups, batches, data)
                    .await?;
            if let Some((mut tx, wallet_ids)) = res {
                for id in wallet_ids {
                    spawn_batch_wallet_accounting(&mut tx, (&data, id)).await?;
                }
                spawn_batch_wallet_signing(&mut tx, BatchWalletSigningData::from(&data)).await?;
                tx.commit().await?;
            }

            Ok::<_, BriaError>(data)
        })
        .await?;
    Ok(())
}

#[job(
    name = "batch_wallet_accounting",
    channel_name = "sync_wallet",
    retries = 20,
    ordered = true
)]
async fn batch_wallet_accounting(
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
            let data: BatchWalletAccountingData =
                data.expect("no BatchWalletAccountingData available");
            batch_wallet_accounting::execute(pool, data, blockchain_cfg, ledger, wallets, batches)
                .await
        })
        .await?;
    Ok(())
}

#[job(name = "batch_wallet_signing", channel_name = "batch", retries = 20)]
async fn batch_wallet_signing(
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
            let data: BatchWalletSigningData = data.expect("no BatchWalletSigningData available");
            let data = batch_wallet_signing::execute(
                pool.clone(),
                data,
                blockchain_cfg,
                ledger,
                wallets,
                batches,
            )
            .await?;

            let mut tx = pool.clone().begin().await?;
            spawn_batch_wallet_finalizing(&mut tx, BatchWalletFinalizingData::from(data.clone()))
                .await?;
            tx.commit().await?;

            Ok::<_, BriaError>(data)
        })
        .await?;
    Ok(())
}

#[job(name = "batch_wallet_finalizing", channel_name = "batch", retries = 20)]
async fn batch_wallet_finalizing(
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
            let data: BatchWalletFinalizingData =
                data.expect("no BatchWalletFinalizingData available");
            batch_wallet_finalizing::execute(pool, data, blockchain_cfg, ledger, wallets, batches)
                .await
        })
        .await?;
    Ok(())
}

#[instrument(name = "job.spawn_sync_all_wallets", skip_all, fields(error, error.level, error.message), err)]
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

#[instrument(name = "job.spawn_sync_wallet", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_sync_wallet(pool: &sqlx::PgPool, data: SyncWalletData) -> Result<(), BriaError> {
    match JobBuilder::new_with_id(Uuid::from(data.wallet_id), "sync_wallet")
        .set_channel_name("wallet_sync")
        .set_channel_args(&data.wallet_id.to_string())
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

#[instrument(name = "job.spawn_process_all_batch_groups", skip_all, fields(error, error.level, error.message), err)]
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

#[instrument(name = "job.spawn_process_batch_group", skip_all, fields(error, error.level, error.message), err)]
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

#[instrument(name = "job.spawn_batch_wallet_accounting", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_batch_wallet_accounting(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    data: impl Into<BatchWalletAccountingData>,
) -> Result<(), BriaError> {
    let data = data.into();
    match batch_wallet_accounting
        .builder()
        .set_json(&data)
        .expect("Couldn't set json")
        .set_channel_args(&data.wallet_id.to_string())
        .spawn(&mut *tx)
        .await
    {
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(name = "job.spawn_batch_wallet_signing", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_batch_wallet_signing(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    data: BatchWalletSigningData,
) -> Result<(), BriaError> {
    match batch_wallet_signing
        .builder()
        .set_json(&data)
        .expect("Couldn't set json")
        .set_channel_name("batch")
        .spawn(&mut *tx)
        .await
    {
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(name = "job.spawn_batch_wallet_finalizing", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_batch_wallet_finalizing(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    data: BatchWalletFinalizingData,
) -> Result<(), BriaError> {
    match batch_wallet_finalizing
        .builder()
        .set_json(&data)
        .expect("Couldn't set json")
        .set_channel_name("batch")
        .spawn(&mut *tx)
        .await
    {
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

impl From<(&ProcessBatchGroupData, WalletId)> for BatchWalletAccountingData {
    fn from((data, wallet_id): (&ProcessBatchGroupData, WalletId)) -> Self {
        Self {
            tracing_data: crate::tracing::extract_tracing_data(),
            account_id: data.account_id,
            batch_id: data.batch_id,
            wallet_id,
        }
    }
}

impl From<&ProcessBatchGroupData> for BatchWalletSigningData {
    fn from(data: &ProcessBatchGroupData) -> Self {
        Self {
            account_id: data.account_id,
            batch_id: data.batch_id,
        }
    }
}

impl From<BatchWalletSigningData> for BatchWalletFinalizingData {
    fn from(data: BatchWalletSigningData) -> Self {
        Self {
            account_id: data.account_id,
            batch_id: data.batch_id,
        }
    }
}
