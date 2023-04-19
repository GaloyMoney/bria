mod batch_finalizing;
mod batch_signing;
mod batch_wallet_accounting;
mod executor;
mod process_batch_group;
mod sync_wallet;

use sqlxmq::{job, CurrentJob, JobBuilder, JobRegistry, OwnedHandle};
use tracing::instrument;
use uuid::{uuid, Uuid};

use crate::{
    app::BlockchainConfig, batch::*, batch_group::*, error::*, ledger::Ledger, payout::*,
    primitives::*, signing_session::*, utxo::Utxos, wallet::*, xpub::*,
};
use batch_finalizing::BatchFinalizingData;
use batch_signing::BatchSigningData;
use batch_wallet_accounting::BatchWalletAccountingData;
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
    xpubs: XPubs,
    batch_groups: BatchGroups,
    batches: Batches,
    signing_sessions: SigningSessions,
    payouts: Payouts,
    ledger: Ledger,
    utxos: Utxos,
    sync_all_wallets_delay: std::time::Duration,
    process_all_batch_groups_delay: std::time::Duration,
    blockchain_cfg: BlockchainConfig,
) -> Result<OwnedHandle, BriaError> {
    let mut registry = JobRegistry::new(&[
        sync_all_wallets,
        sync_wallet,
        process_all_batch_groups,
        schedule_process_batch_group,
        process_batch_group,
        batch_wallet_accounting,
        batch_signing,
        batch_finalizing,
    ]);
    registry.set_context(SyncAllWalletsDelay(sync_all_wallets_delay));
    registry.set_context(ProcessAllBatchesDelay(process_all_batch_groups_delay));
    registry.set_context(blockchain_cfg);
    registry.set_context(wallets);
    registry.set_context(xpubs);
    registry.set_context(batch_groups);
    registry.set_context(batches);
    registry.set_context(signing_sessions);
    registry.set_context(payouts);
    registry.set_context(ledger);
    registry.set_context(utxos);

    Ok(registry.runner(pool).run().await?)
}

#[job(name = "sync_all_wallets")]
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
            for (account_id, wallet_id) in wallets.all_ids().await? {
                let _ = spawn_sync_wallet(&pool, SyncWalletData::new(account_id, wallet_id)).await;
            }
            Ok::<(), BriaError>(())
        })
        .await?;
    spawn_sync_all_wallets(current_job.pool(), delay).await?;
    Ok(())
}

#[job(name = "process_all_batch_groups")]
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
                    let _ = spawn_schedule_process_batch_group(
                        &pool,
                        (group.account_id, group.id),
                        delay
                            .checked_sub(std::time::Duration::from_secs(1))
                            .unwrap_or_default(),
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

#[job(name = "sync_wallet")]
async fn sync_wallet(
    mut current_job: CurrentJob,
    wallets: Wallets,
    blockchain_cfg: BlockchainConfig,
    utxos: Utxos,
    ledger: Ledger,
) -> Result<(), BriaError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: SyncWalletData = data.expect("no SyncWalletData available");
            sync_wallet::execute(pool, wallets, blockchain_cfg, utxos, ledger, data).await
        })
        .await?;
    Ok(())
}

#[job(name = "schedule_process_batch_group")]
async fn schedule_process_batch_group(mut current_job: CurrentJob) -> Result<(), BriaError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let mut data: ProcessBatchGroupData = data.expect("no SyncWalletData available");
            data.tracing_data = crate::tracing::extract_tracing_data();
            onto_account_utxo_queue(
                &pool,
                data.account_id,
                Uuid::new_v4(),
                "process_batch_group",
                data,
            )
            .await
        })
        .await?;
    Ok(())
}

#[job(name = "process_batch_group")]
async fn process_batch_group(
    mut current_job: CurrentJob,
    payouts: Payouts,
    wallets: Wallets,
    utxos: Utxos,
    batch_groups: BatchGroups,
    batches: Batches,
) -> Result<(), BriaError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: ProcessBatchGroupData = data.expect("no ProcessBatchGroupData available");
            let (data, res) = process_batch_group::execute(
                pool,
                payouts,
                wallets,
                batch_groups,
                batches,
                utxos,
                data,
            )
            .await?;
            if let Some((mut tx, wallet_ids)) = res {
                for id in wallet_ids {
                    spawn_batch_wallet_accounting(&mut tx, (&data, id)).await?;
                }
                spawn_batch_signing(tx, &data).await?;
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
    utxos: Utxos,
    batches: Batches,
) -> Result<(), BriaError> {
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: BatchWalletAccountingData =
                data.expect("no BatchWalletAccountingData available");
            batch_wallet_accounting::execute(data, blockchain_cfg, ledger, wallets, utxos, batches)
                .await
        })
        .await?;
    Ok(())
}

#[job(name = "batch_signing", channel_name = "batch_signing", retries = 20)]
async fn batch_signing(
    mut current_job: CurrentJob,
    blockchain_cfg: BlockchainConfig,
    batches: Batches,
    wallets: Wallets,
    xpubs: XPubs,
    signing_sessions: SigningSessions,
) -> Result<(), BriaError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: BatchSigningData = data.expect("no BatchSigningData available");
            let data = batch_signing::execute(
                pool.clone(),
                data,
                blockchain_cfg,
                batches,
                signing_sessions,
                wallets,
                xpubs,
            )
            .await?;

            let mut tx = pool.clone().begin().await?;
            spawn_batch_finalizing(&mut tx, BatchFinalizingData::from(data.clone())).await?;
            tx.commit().await?;

            Ok::<_, BriaError>(data)
        })
        .await?;
    Ok(())
}

#[job(
    name = "batch_finalizing",
    channel_name = "batch_finalizing",
    retries = 20
)]
async fn batch_finalizing(
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
            let data: BatchFinalizingData = data.expect("no BatchFinalizingData available");
            batch_finalizing::execute(pool, data, blockchain_cfg, ledger, wallets, batches).await
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
        .set_channel_name("sync_all_wallets")
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
    onto_account_utxo_queue(pool, data.account_id, data.wallet_id, "sync_wallet", data).await?;
    Ok(())
}

#[instrument(name = "job.spawn_process_all_batch_groups", skip_all, fields(error, error.level, error.message), err)]
pub async fn spawn_process_all_batch_groups(
    pool: &sqlx::PgPool,
    delay: std::time::Duration,
) -> Result<(), BriaError> {
    match JobBuilder::new_with_id(PROCESS_ALL_BATCH_GROUPS_ID, "process_all_batch_groups")
        .set_channel_name("process_all_batch_groups")
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

#[instrument(name = "job.schedule_spawn_process_batch_group", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_schedule_process_batch_group(
    pool: &sqlx::PgPool,
    data: impl Into<ProcessBatchGroupData>,
    delay: std::time::Duration,
) -> Result<(), BriaError> {
    let data = data.into();
    match JobBuilder::new_with_id(
        Uuid::from(data.batch_group_id),
        "schedule_process_batch_group",
    )
    .set_ordered(true)
    .set_channel_name("schedule_batch_group")
    .set_channel_args(&schedule_batch_group_channel_arg(data.batch_group_id))
    .set_delay(delay)
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

#[instrument(name = "job.spawn_batch_signing", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_batch_signing(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    data: impl Into<BatchSigningData>,
) -> Result<(), BriaError> {
    let data = data.into();
    match batch_signing
        .builder()
        .set_json(&data)
        .expect("Couldn't set json")
        .set_ordered(true)
        .set_channel_args(&format!("batch_id:{}", data.batch_id))
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

#[instrument(name = "job.spawn_batch_finalizing", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_batch_finalizing(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    data: BatchFinalizingData,
) -> Result<(), BriaError> {
    match batch_finalizing
        .builder()
        .set_json(&data)
        .expect("Couldn't set json")
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

fn schedule_batch_group_channel_arg(batch_group_id: BatchGroupId) -> String {
    format!("batch_group_id:{batch_group_id}")
}

async fn onto_account_utxo_queue<D: serde::Serialize>(
    pool: &sqlx::PgPool,
    account_id: AccountId,
    uuid: impl Into<Uuid>,
    name: &str,
    data: D,
) -> Result<D, BriaError> {
    match JobBuilder::new_with_id(uuid.into(), name)
        .set_ordered(true)
        .set_channel_name("account_utxos")
        .set_channel_args(&account_utxo_channel_arg(account_id))
        .set_json(&data)
        .expect("Couldn't set json")
        .spawn(pool)
        .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(data),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(BriaError::from(e))
        }
        Ok(_) => Ok(data),
    }
}

fn account_utxo_channel_arg(account_id: AccountId) -> String {
    format!("account_id:{account_id}")
}

impl From<(AccountId, BatchGroupId)> for ProcessBatchGroupData {
    fn from((account_id, batch_group_id): (AccountId, BatchGroupId)) -> Self {
        Self {
            batch_group_id,
            account_id,
            batch_id: BatchId::new(),
            tracing_data: crate::tracing::extract_tracing_data(),
        }
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

impl From<&ProcessBatchGroupData> for BatchSigningData {
    fn from(data: &ProcessBatchGroupData) -> Self {
        Self {
            account_id: data.account_id,
            batch_id: data.batch_id,
            tracing_data: crate::tracing::extract_tracing_data(),
        }
    }
}

impl From<BatchSigningData> for BatchFinalizingData {
    fn from(data: BatchSigningData) -> Self {
        Self {
            account_id: data.account_id,
            batch_id: data.batch_id,
        }
    }
}
