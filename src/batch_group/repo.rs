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
            INSERT INTO batch_groups (id, account_id, name, description, batch_cfg)
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
                 FROM batch_groups
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
}
