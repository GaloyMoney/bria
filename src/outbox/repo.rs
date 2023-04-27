use sqlx::{Pool, Postgres};
use uuid::Uuid;

use std::{collections::HashMap, sync::Arc};

use super::event::*;
use crate::{error::*, primitives::*};

#[derive(Clone)]
pub(super) struct OutboxRepo {
    pool: Pool<Postgres>,
}

impl OutboxRepo {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn persist_event(&self, event: OutboxEvent) -> Result<(), BriaError> {
        sqlx::query!(
            r#"
            INSERT INTO bria_outbox_events
            (id, account_id, sequence, ledger_event_id, ledger_tx_id, payload, recorded_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            Uuid::from(event.id),
            Uuid::from(event.account_id),
            i64::from(event.sequence),
            event.ledger_event_id as Option<SqlxLedgerEventId>,
            event.ledger_tx_id.map(Uuid::from),
            serde_json::to_value(event.payload)?,
            event.recorded_at,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_latest_sequences(
        &self,
    ) -> Result<
        HashMap<AccountId, Arc<tokio::sync::RwLock<(EventSequence, Option<SqlxLedgerEventId>)>>>,
        BriaError,
    > {
        let rows = sqlx::query!(
            r#"
            SELECT account_id, MAX(sequence) AS "sequence!", MAX(ledger_event_id) AS "ledger_event_id: SqlxLedgerEventId"
            FROM bria_outbox_events
            GROUP BY account_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut map = HashMap::new();
        for row in rows {
            map.insert(
                AccountId::from(row.account_id),
                Arc::new(tokio::sync::RwLock::new((
                    EventSequence::from(row.sequence),
                    row.ledger_event_id,
                ))),
            );
        }
        Ok(map)
    }
}
