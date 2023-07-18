use sqlx::{Pool, Postgres};
use tracing::instrument;

use std::collections::HashMap;

use super::{entity::*, error::PayoutQueueError};
use crate::{entity::*, primitives::*};

#[derive(Debug, Clone)]
pub struct PayoutQueues {
    pool: Pool<Postgres>,
}

impl PayoutQueues {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    #[instrument(name = "payout_queues.create", skip(self))]
    pub async fn create(&self, queue: NewPayoutQueue) -> Result<PayoutQueueId, PayoutQueueError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            r#"
            INSERT INTO bria_payout_queues (id, account_id, name)
            VALUES ($1, $2, $3)
            "#,
            queue.id as PayoutQueueId,
            queue.account_id as AccountId,
            queue.name,
        )
        .execute(&mut *tx)
        .await?;
        let id = queue.id;
        EntityEvents::<PayoutQueueEvent>::persist(
            "bria_payout_queue_events",
            &mut tx,
            queue.initial_events().new_serialized_events(id),
        )
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn find_by_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<PayoutQueue, PayoutQueueError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_payout_queues b
              JOIN bria_payout_queue_events e ON b.id = e.id
              WHERE account_id = $1 AND name = $2
              ORDER BY e.sequence"#,
            account_id as AccountId,
            name
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Err(PayoutQueueError::PayoutQueueNameNotFound(name));
        }
        let mut events = EntityEvents::new();
        for row in rows {
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(PayoutQueue::try_from(events)?)
    }

    pub async fn find_by_id(
        &self,
        account_id: AccountId,
        id: PayoutQueueId,
    ) -> Result<PayoutQueue, PayoutQueueError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_payout_queues b
              JOIN bria_payout_queue_events e ON b.id = e.id
              WHERE account_id = $1 AND b.id = $2
              ORDER BY e.sequence"#,
            account_id as AccountId,
            id as PayoutQueueId,
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Err(PayoutQueueError::PayoutQueueIdNotFound(id.to_string()));
        }
        let mut events = EntityEvents::new();
        for row in rows {
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(PayoutQueue::try_from(events)?)
    }
    pub async fn list_by_account_id(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<PayoutQueue>, PayoutQueueError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_payout_queues b
              JOIN bria_payout_queue_events e ON b.id = e.id
              WHERE account_id = $1
              ORDER BY b.id, e.sequence"#,
            account_id as AccountId,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entity_events = HashMap::new();
        for row in rows {
            let id = PayoutQueueId::from(row.id);
            let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(entity_events
            .into_values()
            .map(PayoutQueue::try_from)
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn all(&self) -> Result<Vec<PayoutQueue>, PayoutQueueError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_payout_queues b
              JOIN bria_payout_queue_events e ON b.id = e.id
              ORDER BY b.id, e.sequence"#,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entity_events = HashMap::new();
        for row in rows {
            let id = PayoutQueueId::from(row.id);
            let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(entity_events
            .into_values()
            .map(PayoutQueue::try_from)
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn update(&self, payout_queue: PayoutQueue) -> Result<(), PayoutQueueError> {
        if !payout_queue.events.is_dirty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        EntityEvents::<PayoutQueueEvent>::persist(
            "bria_payout_queue_events",
            &mut tx,
            payout_queue.events.new_serialized_events(payout_queue.id),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
