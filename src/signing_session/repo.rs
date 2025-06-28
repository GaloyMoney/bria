use es_entity::*;
use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use super::{entity::*, error::SigningSessionError};
use crate::primitives::*;
use std::collections::HashMap;

#[derive(EsRepo, Clone)]
#[es_repo(
    entity = "SigningSession",
    err = "SigningSessionError",
    columns(
        batch_id(ty = "BatchId", update(persist = false)),
        account_id(ty = "AccountId", update(persist = false)),
        xpub_fingerprint(ty = "XPubFingerprint", update(persist = false))
    ),
    tbl_prefix = "bria"
)]

pub struct SigningSessions {
    pool: Pool<Postgres>,
}

impl SigningSessions {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn persist_sessions(
        &self,
        sessions: HashMap<XPubFingerprint, NewSigningSession>,
    ) -> Result<BatchSigningSession, SigningSessionError> {
        let (account_id, batch_id) = if let Some((_, first_session)) = sessions.iter().next() {
            (first_session.account_id, first_session.batch_id)
        } else {
            return Err(SigningSessionError::EsEntityError(
                es_entity::EsEntityError::NotFound,
            ));
        };
        self.create_all(sessions.into_values().collect()).await?;
        Ok(self
            .list_for_batch(account_id, batch_id)
            .await?
            .expect("New session not found"))
    }

    pub async fn update_sessions(
        &self,
        op: &mut DbOp<'_>,
        sessions: &HashMap<XPubFingerprint, SigningSession>,
    ) -> Result<(), SigningSessionError> {
        let mut events: Vec<EntityEvents<SigningSessionEvent>> = sessions
            .values()
            .map(|session| session.events.clone())
            .collect();
        self.persist_events_batch(op, &mut events);
        Ok(())
    }

    pub async fn list_for_batch(
        &self,
        account_id: AccountId,
        batch_id: BatchId,
    ) -> Result<Option<BatchSigningSession>, SigningSessionError> {
        let mut signing_sessions = Vec::new();
        let mut query = es_entity::PaginatedQueryArgs::<
            signing_session_cursor::SigningSessionsByCreatedAtCursor,
        > {
            first: Default::default(),
            after: None,
        };

        loop {
            let es_entity::PaginatedQueryArgs { first, after } = query;
            let (id, created_at) = if let Some(after) = after {
                (Some(after.id), Some(after.created_at))
            } else {
                (None, None)
            };

            let (entities, has_next_page) = es_entity::es_query!(
                "bria",
                &self.pool,
                r#"
                SELECT *
                FROM bria_signing_sessions
                WHERE account_id = $1 AND batch_id = $2
                AND (COALESCE((created_at, id) > ($4, $3), $3 IS NULL))
                ORDER BY created_at, id
                FOR UPDATE"#,
                account_id as AccountId,
                batch_id as BatchId,
                id as Option<SigningSessionId>,
                created_at as Option<chrono::DateTime<chrono::Utc>>,
            )
            .fetch_n(first)
            .await?;

            signing_sessions.extend(entities);

            if !has_next_page {
                break;
            }
            let end_cursor = signing_sessions
                .last()
                .map(signing_session_cursor::SigningSessionsByCreatedAtCursor::from);

            query.after = end_cursor;
        }
        let mut xpub_sessions = HashMap::new();
        for session in signing_sessions {
            xpub_sessions.insert(session.xpub_fingerprint, session);
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
        xpub_fingerprint: XPubFingerprint,
    ) -> Result<Vec<BatchId>, SigningSessionError> {
        let rows = sqlx::query!(
            r#"
          SELECT batch_id
          FROM bria_signing_sessions
          WHERE account_id = $1 AND xpub_fingerprint = $2 FOR UPDATE"#,
            Uuid::from(account_id),
            xpub_fingerprint.as_bytes()
        )
        .fetch_all(&mut **tx)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| BatchId::from(row.batch_id))
            .collect())
    }
}
