use sqlx::{Pool, Postgres};
use uuid::Uuid;

use std::collections::HashMap;

use super::entity::*;
use crate::{entity::EntityEvents, error::*, primitives::*};

#[derive(Clone)]
pub struct SigningSessions {
    pool: Pool<Postgres>,
}

impl SigningSessions {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn find_for_batch(
        &self,
        batch_id: BatchId,
    ) -> Result<BatchSigningSession, BriaError> {
        let entity_events = {
            let rows = sqlx::query!(
                r#"
              SELECT b.*, e.sequence, e.event_type, e.event as "event?"
              FROM bria_signing_session b
              JOIN bria_signing_session_events e ON b.id = e.id
              WHERE batch_id = $1
              ORDER BY b.id, sequence"#,
                Uuid::from(batch_id)
            )
            .fetch_all(&self.pool)
            .await?;
            let mut entity_events = HashMap::new();
            for mut row in rows {
                let id = SigningSessionId::from(row.id);
                let sequence = row.sequence;
                let event = row.event.take().expect("Missing event");
                let (_, events) = entity_events
                    .entry(id)
                    .or_insert((row, EntityEvents::<SigningSessionEvent>::new()));
                events.load_event(sequence as usize, event)?;
            }
            entity_events
        };
        let mut xpub_sessions = HashMap::new();
        for (id, (first_row, events)) in entity_events {
            let xpub_id = XPubId::from(bitcoin::Fingerprint::from(
                first_row.xpub_fingerprint.as_ref(),
            ));
            let session = SigningSession {
                id: SigningSessionId::from(id),
                account_id: AccountId::from(first_row.account_id),
                batch_id,
                xpub_id,
                events,
            };
            xpub_sessions.insert(xpub_id, session);
        }
        Ok(BatchSigningSession { xpub_sessions })
    }
}
