mod executor;
mod sync_keychain;

use sqlxmq::{job, CurrentJob, JobBuilder, JobRegistry, OwnedHandle};
use tracing::instrument;
use uuid::{uuid, Uuid};

use crate::{error::*, primitives::*, wallet::*};
pub use executor::JobExecutionError;
use executor::JobExecutor;

const SYNC_ALL_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");

#[derive(Debug, Clone)]
struct SyncAllDelay(std::time::Duration);

pub async fn start_job_runner(
    pool: &sqlx::PgPool,
    wallets: Wallets,
    sync_all_delay: std::time::Duration,
) -> Result<OwnedHandle, BriaError> {
    let mut registry = JobRegistry::new(&[sync_all_wallets, sync_keychain]);
    registry.set_context(SyncAllDelay(sync_all_delay));
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
            for id in wallets.all_keychain_ids().await? {
                let _ = spawn_sync_keychain(&pool, id).await;
            }
            Ok::<(), BriaError>(())
        })
        .await?;
    spawn_sync_all_wallets(current_job.pool(), delay).await?;
    Ok(())
}

#[job(name = "sync_keychain", channel_name = "wallet", retries = 20)]
async fn sync_keychain(mut current_job: CurrentJob, wallets: Wallets) -> Result<(), BriaError> {
    let keychain_id = KeychainId::from(current_job.id());
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|_| async move { sync_keychain::execute(wallets, keychain_id).await })
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
pub async fn spawn_sync_keychain(pool: &sqlx::PgPool, id: KeychainId) -> Result<(), BriaError> {
    match JobBuilder::new_with_id(Uuid::from(id), "sync_keychain")
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
