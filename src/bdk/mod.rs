pub mod error;
pub(crate) mod pg;

pub async fn last_sync_time(pool: &sqlx::PgPool) -> Result<u32, error::BdkError> {
    pg::SyncTimes::last_sync_time(pool).await
}
