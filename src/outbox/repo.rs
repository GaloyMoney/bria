use sqlx::{Pool, Postgres, QueryBuilder};
use uuid::Uuid;

use std::{collections::HashMap, sync::Arc};

use super::{error::OutboxError, event::*};
use crate::primitives::*;

#[derive(Clone)]
pub(super) struct OutboxRepo {
    pool: Pool<Postgres>,
}

impl OutboxRepo {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn persist_events<T>(&self, events: &[OutboxEvent<T>]) -> Result<(), OutboxError> {
        if events.is_empty() {
            return Ok(());
        }

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"INSERT INTO bria_outbox_events
            (id, account_id, sequence, ledger_event_id, ledger_tx_id, payload, recorded_at)"#,
        );
        query_builder.push_values(events.iter(), |mut builder, event| {
            builder.push_bind(event.id);
            builder.push_bind(event.account_id);
            builder.push_bind(event.sequence);
            builder.push_bind(event.ledger_event_id);
            builder.push_bind(event.ledger_tx_id);
            builder.push_bind(
                serde_json::to_value(event.payload.clone()).expect("Could not serialize payload"),
            );
            builder.push_bind(event.recorded_at);
        });
        let query = query_builder.build();
        query.execute(&self.pool).await?;
        Ok(())
    }

    pub async fn load_next_page(
        &self,
        account_id: AccountId,
        sequence: EventSequence,
        buffer_size: usize,
    ) -> Result<Vec<OutboxEvent<WithoutAugmentation>>, OutboxError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, account_id, sequence AS "sequence: EventSequence", ledger_event_id AS "ledger_event_id: SqlxLedgerEventId", ledger_tx_id, payload, recorded_at
            FROM bria_outbox_events
            WHERE account_id = $1 AND sequence > $2
            ORDER BY sequence ASC
            LIMIT $3
            "#,
            Uuid::from(account_id),
            sequence as EventSequence,
            buffer_size as i64,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut events = Vec::new();
        for row in rows {
            events.push(OutboxEvent {
                id: OutboxEventId::from(row.id),
                account_id: AccountId::from(row.account_id),
                sequence: row.sequence,
                ledger_event_id: row.ledger_event_id,
                ledger_tx_id: row.ledger_tx_id.map(LedgerTransactionId::from),
                payload: serde_json::from_value(row.payload)?,
                recorded_at: row.recorded_at,
                augmentation: None,
            });
        }
        Ok(events)
    }

    pub async fn load_latest_sequences(
        &self,
    ) -> Result<
        HashMap<AccountId, Arc<tokio::sync::RwLock<(EventSequence, Option<SqlxLedgerEventId>)>>>,
        OutboxError,
    > {
        let rows = sqlx::query!(
            r#"
            SELECT account_id, MAX(sequence) AS "sequence!: EventSequence", MAX(ledger_event_id) AS "ledger_event_id: SqlxLedgerEventId"
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
                    row.sequence,
                    row.ledger_event_id,
                ))),
            );
        }
        Ok(map)
    }
}
