use sqlx::{Pool, Postgres, QueryBuilder};
use uuid::Uuid;

use std::collections::HashMap;

use super::entity::*;
use crate::{entity::EntityEvents, error::*, primitives::*};

#[derive(Clone)]
pub struct SigningSessions {
    pool: Pool<Postgres>,
}

impl SigningSessions {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn persist_new_sessions(
        &self,
        sessions: HashMap<XPubId, NewSigningSession>,
    ) -> Result<BatchSigningSession, BriaError> {
        let mut tx = self.pool.begin().await?;
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"INSERT INTO bria_signing_session
            (id, account_id, batch_id, xpub_fingerprint, unsigned_psbt)"#,
        );
        query_builder.push_values(sessions, |mut builder, (xpub_id, session)| {
            builder.push_bind(Uuid::from(session.id));
            builder.push_bind(Uuid::from(session.account_id));
            builder.push_bind(Uuid::from(session.batch_id));
            builder.push_bind(xpub_id.as_bytes().to_owned());
            builder.push_bind(bitcoin::consensus::encode::serialize(
                &session.unsigned_psbt,
            ));
        });
        let query = query_builder.build();
        query.execute(&mut tx).await?;
        unimplemented!()
    }

    pub async fn find_for_batch(
        &self,
        account_id: AccountId,
        batch_id: BatchId,
    ) -> Result<Option<BatchSigningSession>, BriaError> {
        let entity_events = {
            let rows = sqlx::query!(
                r#"
              SELECT b.*, e.sequence, e.event_type, e.event as "event?"
              FROM bria_signing_session b
              JOIN bria_signing_session_events e ON b.id = e.id
              WHERE account_id = $1 AND batch_id = $2
              ORDER BY b.id, sequence"#,
                Uuid::from(account_id),
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
                    .or_insert_with(|| (row, EntityEvents::new()));
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
                unsigned_psbt: bitcoin::consensus::deserialize(&first_row.unsigned_psbt)?,
                events,
            };
            xpub_sessions.insert(xpub_id, session);
        }
        if xpub_sessions.is_empty() {
            Ok(None)
        } else {
            Ok(Some(BatchSigningSession { xpub_sessions }))
        }
    }
}
