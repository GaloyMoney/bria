use sqlx::{Pool, Postgres};
use tracing::instrument;
use uuid::Uuid;

use super::entity::*;
use crate::{error::*, primitives::*};

#[derive(Debug, Clone)]
pub struct BatchGroups {
    pool: Pool<Postgres>,
}

impl BatchGroups {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    #[instrument(name = "batch_groups.create", skip(self))]
    pub async fn create(&self, group: NewBatchGroup) -> Result<BatchGroupId, BriaError> {
        let NewBatchGroup {
            id,
            account_id,
            name,
            description,
            config,
        } = group;
        sqlx::query!(
            r#"
            INSERT INTO bria_batch_groups (id, account_id, name, description, batch_cfg)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            Uuid::from(id),
            Uuid::from(account_id),
            name,
            description,
            serde_json::to_value(config)?,
        )
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn find_by_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<BatchGroupId, BriaError> {
        let record = sqlx::query!(
            r#"SElECT id
                 FROM bria_batch_groups
                 WHERE account_id = $1 AND name = $2 ORDER BY version DESC LIMIT 1"#,
            Uuid::from(account_id),
            name
        )
        .fetch_optional(&self.pool)
        .await?;
        if record.is_none() {
            return Err(BriaError::BatchGroupNotFound);
        }

        Ok(BatchGroupId::from(record.unwrap().id))
    }

    pub async fn find_by_id(&self, id: BatchGroupId) -> Result<BatchGroup, BriaError> {
        let record = sqlx::query!(
            r#"SElECT id, account_id, name, batch_cfg
                 FROM bria_batch_groups
                 WHERE id = $1 ORDER BY version DESC LIMIT 1"#,
            Uuid::from(id),
        )
        .fetch_optional(&self.pool)
        .await?;

        record
            .map(|row| BatchGroup {
                id: BatchGroupId::from(row.id),
                account_id: AccountId::from(row.account_id),
                name: row.name,
                config: serde_json::from_value(row.batch_cfg)
                    .expect("Couldn't deserialize batch config"),
            })
            .ok_or(BriaError::BatchGroupNotFound)
    }

    pub async fn all(&self) -> Result<impl Iterator<Item = BatchGroup>, BriaError> {
        let rows = sqlx::query!(
            r#"WITH latest AS (
                 SELECT DISTINCT(id), MAX(version) OVER (PARTITION BY id ORDER BY version DESC)
                 FROM bria_batch_groups
               ) SELECT id, account_id, name, batch_cfg FROM bria_batch_groups
                 WHERE (id, version) IN (SELECT * FROM latest)"#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|row| BatchGroup {
            id: BatchGroupId::from(row.id),
            account_id: AccountId::from(row.account_id),
            name: row.name,
            config: serde_json::from_value(row.batch_cfg)
                .expect("Couldn't deserialize batch config"),
        }))
    }
}
