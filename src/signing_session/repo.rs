use sqlx::{Pool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

use std::collections::HashMap;

use super::{entity::*, error::SigningSessionError};
use crate::{entity::EntityEvents, primitives::*};

#[derive(Clone)]
pub struct SigningSessions {
    pool: Pool<Postgres>,
}

impl SigningSessions {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn persist_sessions(
        &self,
        sessions: HashMap<XPubId, NewSigningSession>,
    ) -> Result<BatchSigningSession, SigningSessionError> {
        let mut tx = self.pool.begin().await?;
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"INSERT INTO bria_signing_sessions
            (id, account_id, batch_id, xpub_fingerprint)"#,
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
        });
        let query = query_builder.build();
        query.execute(&mut *tx).await?;
        EntityEvents::<SigningSessionEvent>::persist(
            "bria_signing_session_events",
            &mut tx,
            sessions.into_values().flat_map(|session| {
                let id = session.id;
                session.initial_events().into_new_serialized_events(id)
            }),
        )
        .await?;
        tx.commit().await?;
        if let (Some(account_id), Some(batch_id)) = (account_id, batch_id) {
            Ok(self
                .list_for_batch(account_id, batch_id)
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
    ) -> Result<(), SigningSessionError> {
        EntityEvents::<SigningSessionEvent>::persist(
            "bria_signing_session_events",
            tx,
            sessions
                .values()
                .flat_map(|session| session.events.new_serialized_events(session.id)),
        )
        .await?;
        Ok(())
    }

    pub async fn list_for_batch(
        &self,
        account_id: AccountId,
        batch_id: BatchId,
    ) -> Result<Option<BatchSigningSession>, SigningSessionError> {
        let entity_events = {
            let rows = sqlx::query!(
                r#"
              SELECT b.*, e.sequence, e.event_type, e.event
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
            for row in rows {
                let id = SigningSessionId::from(row.id);
                let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
                events.load_event(row.sequence as usize, row.event)?;
            }
            entity_events
        };
        let mut xpub_sessions = HashMap::new();
        for (_, events) in entity_events {
            let session = SigningSession::try_from(events)?;
            xpub_sessions.insert(session.xpub_id, session);
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
    ) -> Result<Vec<BatchId>, SigningSessionError> {
        let rows = sqlx::query!(
            r#"
          SELECT batch_id
          FROM bria_signing_sessions
          WHERE account_id = $1 AND xpub_fingerprint = $2 FOR UPDATE"#,
            Uuid::from(account_id),
            xpub_id.as_bytes()
        )
        .fetch_all(&mut **tx)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| BatchId::from(row.batch_id))
            .collect())
    }
}
