use sqlx::{Pool, Postgres, QueryBuilder, Transaction};
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
            r#"INSERT INTO bria_signing_sessions
            (id, account_id, batch_id, xpub_fingerprint, unsigned_psbt)"#,
        );
        let mut account_id = None;
        let mut batch_id = None;
        query_builder.push_values(sessions.iter(), |mut builder, (xpub_id, session)| {
            if account_id.is_none() && batch_id.is_none() {
                account_id = Some(session.account_id);
                batch_id = Some(session.batch_id);
            }
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
        query_builder = sqlx::QueryBuilder::new(
            r#"INSERT INTO bria_signing_session_events
            (id, sequence, event_type, event)"#,
        );
        let initial_events = NewSigningSession::initial_events();
        query_builder.push_values(
            sessions
                .values()
                .flat_map(|session| initial_events.new_serialized_events(session.id)),
            |mut builder, (id, sequence, event_type, event)| {
                builder.push_bind(id);
                builder.push_bind(sequence);
                builder.push_bind(event_type);
                builder.push_bind(event);
            },
        );
        let query = query_builder.build();
        query.execute(&mut tx).await?;
        tx.commit().await?;
        if let (Some(account_id), Some(batch_id)) = (account_id, batch_id) {
            Ok(self
                .find_for_batch(account_id, batch_id)
                .await?
                .expect("New session not found"))
        } else {
            unreachable!()
        }
    }

    pub async fn update_sessions(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        sessions: &HashMap<XPubId, SigningSession>,
    ) -> Result<(), BriaError> {
        let mut query_builder = sqlx::QueryBuilder::new(
            r#"INSERT INTO bria_signing_session_events
            (id, sequence, event_type, event)"#,
        );
        query_builder.push_values(
            sessions
                .values()
                .flat_map(|session| session.events.new_serialized_events(session.id)),
            |mut builder, (id, sequence, event_type, event)| {
                builder.push_bind(id);
                builder.push_bind(sequence);
                builder.push_bind(event_type);
                builder.push_bind(event);
            },
        );
        let query = query_builder.build();
        query.execute(&mut *tx).await?;
        Ok(())
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
              FROM bria_signing_sessions b
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
                id,
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

    pub async fn list_batch_ids_for(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: AccountId,
        xpub_id: XPubId,
    ) -> Result<Vec<BatchId>, BriaError> {
        let rows = sqlx::query!(
            r#"
          SELECT batch_id
          FROM bria_signing_sessions
          WHERE account_id = $1 AND xpub_fingerprint = $2 FOR UPDATE"#,
            Uuid::from(account_id),
            xpub_id.as_bytes()
        )
        .fetch_all(&mut *tx)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| BatchId::from(row.batch_id))
            .collect())
    }
}
