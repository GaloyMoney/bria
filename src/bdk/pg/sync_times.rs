use bdk::database::SyncTime;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{bdk::error::BdkError, primitives::*};

pub struct SyncTimes {
    keychain_id: KeychainId,
    pool: PgPool,
}

impl SyncTimes {
    pub fn new(keychain_id: KeychainId, pool: PgPool) -> Self {
        Self { keychain_id, pool }
    }

    pub async fn persist(&self, time: SyncTime) -> Result<(), bdk::Error> {
        sqlx::query!(
            r#"INSERT INTO bdk_sync_times (keychain_id, height, timestamp)
            VALUES ($1, $2, $3)
            ON CONFLICT (keychain_id) DO UPDATE SET height = EXCLUDED.height, timestamp = EXCLUDED.timestamp, modified_at = NOW()"#,
            Uuid::from(self.keychain_id),
            time.block_time.height as f64,
            time.block_time.timestamp as f64,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(())
    }

    pub async fn get(&self) -> Result<Option<SyncTime>, bdk::Error> {
        let sync_time = sqlx::query!(
            r#"SELECT height, timestamp FROM bdk_sync_times WHERE keychain_id = $1"#,
            Uuid::from(self.keychain_id),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(sync_time.map(|time| SyncTime {
            block_time: bdk::BlockTime {
                height: time.height as u32,
                timestamp: time.timestamp as u64,
            },
        }))
    }

    pub async fn last_sync_time(pool: &PgPool) -> Result<u32, BdkError> {
        let sync_time =
            sqlx::query!(r#"SELECT COALESCE(MAX(height), 0) as "height!" FROM bdk_sync_times"#,)
                .fetch_one(pool)
                .await?;
        Ok(sync_time.height as u32)
    }
}
