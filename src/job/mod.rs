mod executor;
mod sync_all_wallets;

use sqlxmq::{job, CurrentJob, JobBuilder, JobRegistry, OwnedHandle};
use tracing::instrument;
use uuid::{uuid, Uuid};

use crate::error::*;
pub use executor::JobExecutionError;
use executor::JobExecutor;

const SYNC_ALL_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");

#[derive(Debug, Clone)]
struct SyncAllDelay(std::time::Duration);

pub async fn start_job_runner(pool: &sqlx::PgPool) -> Result<OwnedHandle, BriaError> {
    let registry = JobRegistry::new(&[sync_all_wallets]);

    Ok(registry.runner(pool).run().await?)
}

#[job(name = "sync_all_wallets", channel_name = "wallet", retries = 20)]
async fn sync_all_wallets(
    mut current_job: CurrentJob,
    SyncAllDelay(delay): SyncAllDelay,
) -> Result<(), BriaError> {
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|_| async move { sync_all_wallets::execute().await })
        .await?;
    spawn_sync_all_wallets(current_job.pool(), delay).await?;
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
