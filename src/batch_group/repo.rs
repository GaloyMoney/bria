use sqlx::{Pool, Postgres};
use tracing::instrument;
use uuid::Uuid;

use std::collections::HashMap;

use super::entity::*;
use crate::{entity::*, error::*, primitives::*};

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
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            r#"
            INSERT INTO bria_batch_groups (id, account_id, name)
            VALUES ($1, $2, $3)
            "#,
            Uuid::from(group.id),
            Uuid::from(group.account_id),
            group.name,
        )
        .execute(&mut tx)
        .await?;
        let id = group.id;
        EntityEvents::<BatchGroupEvent>::persist(
            "bria_batch_group_events",
            &mut tx,
            group.initial_events().new_serialized_events(id),
        )
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn find_by_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<BatchGroup, BriaError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_batch_groups b
              JOIN bria_batch_group_events e ON b.id = e.id
              WHERE account_id = $1 AND name = $2
              ORDER BY e.sequence"#,
            Uuid::from(account_id),
            name
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Err(BriaError::BatchGroupNotFound);
        }
        let mut events = EntityEvents::new();
        for row in rows {
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(BatchGroup::try_from(events)?)
    }

    pub async fn find_by_id(
        &self,
        account_id: AccountId,
        id: BatchGroupId,
    ) -> Result<BatchGroup, BriaError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_batch_groups b
              JOIN bria_batch_group_events e ON b.id = e.id
              WHERE account_id = $1 AND b.id = $2
              ORDER BY e.sequence"#,
            Uuid::from(account_id),
            Uuid::from(id)
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Err(BriaError::BatchGroupNotFound);
        }
        let mut events = EntityEvents::new();
        for row in rows {
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(BatchGroup::try_from(events)?)
    }
    pub async fn list_by_account_id(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<BatchGroup>, BriaError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_batch_groups b
              JOIN bria_batch_group_events e ON b.id = e.id
              WHERE account_id = $1
              ORDER BY b.id, e.sequence"#,
            account_id as AccountId,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entity_events = HashMap::new();
        for row in rows {
            let id = BatchGroupId::from(row.id);
            let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(entity_events
            .into_values()
            .map(BatchGroup::try_from)
            .collect::<Result<Vec<_>, _>>()?)
    }
    pub async fn all(&self) -> Result<Vec<BatchGroup>, BriaError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_batch_groups b
              JOIN bria_batch_group_events e ON b.id = e.id
              ORDER BY b.id, e.sequence"#,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entity_events = HashMap::new();
        for row in rows {
            let id = BatchGroupId::from(row.id);
            let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(entity_events
            .into_values()
            .map(BatchGroup::try_from)
            .collect::<Result<Vec<_>, _>>()?)
    }
}
