mod executor;
mod sync_wallet;

use sqlxmq::{job, CurrentJob, JobBuilder, JobRegistry, OwnedHandle};
use tracing::instrument;
use uuid::{uuid, Uuid};

use crate::{app::BlockchainConfig, error::*, primitives::*, wallet::*};
pub use executor::JobExecutionError;
use executor::JobExecutor;

const SYNC_ALL_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");

#[derive(Debug, Clone)]
struct SyncAllDelay(std::time::Duration);

pub async fn start_job_runner(
    pool: &sqlx::PgPool,
    wallets: Wallets,
    sync_all_delay: std::time::Duration,
    blockchain_cfg: BlockchainConfig,
) -> Result<OwnedHandle, BriaError> {
    let mut registry = JobRegistry::new(&[sync_all_wallets, sync_wallet]);
    registry.set_context(SyncAllDelay(sync_all_delay));
    registry.set_context(blockchain_cfg);
    registry.set_context(wallets);

    Ok(registry.runner(pool).run().await?)
}

#[job(name = "sync_all_wallets", channel_name = "wallet", retries = 20)]
async fn sync_all_wallets(
    mut current_job: CurrentJob,
    wallets: Wallets,
    SyncAllDelay(delay): SyncAllDelay,
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

#[job(name = "sync_wallet", channel_name = "wallet", retries = 20)]
async fn sync_wallet(
    mut current_job: CurrentJob,
    wallets: Wallets,
    blockchain_cfg: BlockchainConfig,
) -> Result<(), BriaError> {
    let wallet_id = WalletId::from(current_job.id());
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(
            |_| async move { sync_wallet::execute(pool, wallets, wallet_id, blockchain_cfg).await },
        )
        .await?;
    Ok(())
}

#[instrument(skip_all, fields(error, error.level, error.message), err)]
pub async fn spawn_sync_all_wallets(
    pool: &sqlx::PgPool,
    duration: std::time::Duration,
) -> Result<(), BriaError> {
    match JobBuilder::new_with_id(SYNC_ALL_ID, "sync_all_wallets")
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
pub async fn spawn_sync_wallet(pool: &sqlx::PgPool, id: WalletId) -> Result<(), BriaError> {
    match JobBuilder::new_with_id(Uuid::from(id), "sync_wallet")
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
