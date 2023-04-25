use sqlx::{Pool, Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

use super::{entity::*, reference::*, value::*};
use crate::{entity::*, error::*, primitives::*};

#[derive(Clone)]
pub struct XPubs {
    pool: Pool<Postgres>,
}

impl XPubs {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    #[instrument(name = "xpubs.persist", skip(self))]
    pub async fn persist(&self, xpub: NewAccountXPub) -> Result<XPubId, BriaError> {
        let id = xpub.id();
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query!(
            r#"INSERT INTO bria_xpubs
            (account_id, name, original, xpub, derivation_path, fingerprint, parent_fingerprint)
            VALUES ((SELECT id FROM bria_accounts WHERE id = $1), $2, $3, $4, $5, $6, $7)
            RETURNING id"#,
            Uuid::from(xpub.account_id),
            xpub.key_name,
            xpub.original,
            &xpub.value.inner.encode(),
            xpub.value.derivation.as_ref().map(|d| d.to_string()),
            id.as_bytes(),
            xpub.value.inner.parent_fingerprint.as_bytes(),
        )
        .fetch_one(&mut tx)
        .await?;
        EntityEvents::<XPubEvent>::persist(
            "bria_xpub_events",
            &mut tx,
            NewAccountXPub::initial_events().new_serialized_events(row.id),
        )
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn persist_updated(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        xpub: AccountXPub,
    ) -> Result<(), BriaError> {
        Ok(EntityEvents::<XPubEvent>::persist(
            "bria_xpub_events",
            tx,
            xpub.events.new_serialized_events(xpub.db_uuid),
        )
        .await?)
    }

    pub async fn find_from_ref(
        &self,
        account_id: AccountId,
        xpub_ref: impl Into<XPubRef>,
    ) -> Result<AccountXPub, BriaError> {
        let xpub_ref = xpub_ref.into();
        let mut tx = self.pool.begin().await?;
        let (id, name, derivation_path, bytes) = match xpub_ref {
            XPubRef::Id(fp) => {
                let record = sqlx::query!(
                    r#"SELECT id, name, derivation_path, xpub
                       FROM bria_xpubs
                       WHERE account_id = $1 AND fingerprint = $2"#,
                    Uuid::from(account_id),
                    fp.as_bytes()
                )
                .fetch_one(&mut tx)
                .await?;
                (record.id, record.name, record.derivation_path, record.xpub)
            }
            XPubRef::Name(name) => {
                let record = sqlx::query!(
                    r#"SELECT id, name, derivation_path,  xpub
                       FROM bria_xpubs
                       WHERE account_id = $1 AND name = $2"#,
                    Uuid::from(account_id),
                    name
                )
                .fetch_one(&mut tx)
                .await?;
                (record.id, record.name, record.derivation_path, record.xpub)
            }
        };

        let rows = sqlx::query!(
            r#"SELECT sequence, event_type, event FROM bria_xpub_events
               WHERE id = $1
               ORDER BY sequence"#,
            id
        )
        .fetch_all(&mut tx)
        .await?;
        let mut events = EntityEvents::new();
        for row in rows {
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(AccountXPub {
            db_uuid: id,
            account_id,
            key_name: name,
            value: XPub {
                derivation: derivation_path
                    .map(|d| d.parse().expect("Couldn't decode derivation path")),
                inner: bitcoin::ExtendedPubKey::decode(&bytes).expect("Couldn't decode xpub"),
            },
            events,
        })
    }
}
