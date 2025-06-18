use es_entity::*;
use sqlx::{Pool, Postgres, Transaction};
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

use super::{entity::*, error::XpubError, reference::*, signer_config::*};
use crate::primitives::*;

#[derive(EsRepo, Clone, Debug)]
#[es_repo(
    entity = "Xpub",
    err = "XpubError",
    columns(
        account_id(ty = "AccountId", list_for),
        name(ty = "String"),
        // this or [u8;4] and have fingerprint in Xpub too?
        fingerprint(ty = "XPubId", update(persist = false))
    ),
    tbl_prefix = "bria"
)]
pub struct XPubs {
    pool: Pool<Postgres>,
}

impl XPubs {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    #[instrument(name = "xpubs.persist", skip(self))]
    pub async fn persist(&self, xpub: NewXpub) -> Result<XPubId, XpubError> {
        let mut tx = self.pool.begin().await?;
        let ret = self.persist_in_tx(&mut tx, xpub).await?;
        tx.commit().await?;
        Ok(ret)
    }

    #[instrument(name = "xpubs.persist_in_tx", skip(self))]
    pub async fn persist_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        xpub: NewXpub,
    ) -> Result<XPubId, XpubError> {
        let xpub_id = xpub.value.id();
        sqlx::query!(
            r#"INSERT INTO bria_xpubs
            (id, account_id, name, fingerprint)
            VALUES ($1, $2, $3, $4)"#,
            xpub.db_uuid,
            Uuid::from(xpub.account_id),
            xpub.name,
            xpub_id.as_bytes()
        )
        .execute(&mut **tx)
        .await?;
        let id = xpub.db_uuid;
        EntityEvents::<XpubEvent>::persist(
            "bria_xpub_events",
            &mut *tx,
            xpub.initial_events().new_serialized_events(id),
        )
        .await?;
        Ok(xpub_id)
    }

    pub async fn persist_updated(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        xpub: Xpub,
    ) -> Result<(), XpubError> {
        if xpub.events.is_dirty() {
            EntityEvents::<XpubEvent>::persist(
                "bria_xpub_events",
                tx,
                xpub.events.new_serialized_events(xpub.db_uuid),
            )
            .await?;
        }

        if let Some((cypher, nonce)) = xpub.encrypted_signer_config {
            let cypher_bytes = &cypher.0;
            let nonce_bytes = &nonce.0;

            sqlx::query!(
                r#"
                INSERT INTO bria_xpub_signer_configs (id, cypher, nonce, created_at, modified_at)
                VALUES ($1, $2, $3, NOW(), NOW())
                ON CONFLICT (id) DO UPDATE
                SET cypher = $2, nonce = $3, modified_at = NOW()
                "#,
                xpub.db_uuid,
                cypher_bytes,
                nonce_bytes,
            )
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

    pub async fn find_from_ref(
        &self,
        account_id: AccountId,
        xpub_ref: impl Into<XPubRef>,
    ) -> Result<Xpub, XpubError> {
        // let xpub_ref = xpub_ref.into();
        // let mut tx = self.pool.begin().await?;
        // let db_uuid = match xpub_ref {
        //     XPubRef::Id(fp) => {
        //         let record = sqlx::query!(
        //             r#"SELECT id FROM bria_xpubs WHERE account_id = $1 AND fingerprint = $2"#,
        //             Uuid::from(account_id),
        //             fp.as_bytes()
        //         )
        //         .fetch_one(&mut *tx)
        //         .await?;
        //         record.id
        //     }
        //     XPubRef::Name(name) => {
        //         let record = sqlx::query!(
        //             r#"SELECT id FROM bria_xpubs WHERE account_id = $1 AND name = $2"#,
        //             Uuid::from(account_id),
        //             name
        //         )
        //         .fetch_one(&mut *tx)
        //         .await?;
        //         record.id
        //     }
        // };

        // let rows = sqlx::query!(
        //     r#"SELECT sequence, event_type, event FROM bria_xpub_events
        //        WHERE id = $1
        //        ORDER BY sequence"#,
        //     db_uuid
        // )
        // .fetch_all(&mut *tx)
        // .await?;
        // let mut events = EntityEvents::new();
        // for row in rows {
        //     events.load_event(row.sequence as usize, row.event)?;
        // }

        // let config_row = sqlx::query!(
        //     r#"
        //     SELECT cypher, nonce
        //     FROM bria_xpub_signer_configs
        //     WHERE id = $1
        //     "#,
        //     db_uuid
        // )
        // .fetch_optional(&self.pool)
        // .await?;

        // let config = match config_row {
        //     Some(row) => Some((ConfigCyper(row.cypher), Nonce(row.nonce))),
        //     None => None,
        // };

        // Ok(Xpub::try_from((events, config))?)
        !unimplemented!();
    }

    pub async fn list_xpubs(&self, account_id: AccountId) -> Result<Vec<Xpub>, XpubError> {
        // let rows = sqlx::query!(
        //     r#"SELECT b.*, e.sequence, e.event
        //     FROM bria_xpubs b
        //     JOIN bria_xpub_events e ON b.id = e.id
        //     WHERE account_id = $1
        //     ORDER BY b.id, e.sequence"#,
        //     account_id as AccountId,
        // )
        // .fetch_all(&self.pool)
        // .await?;

        // let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();

        // let config_rows = sqlx::query!(
        //     r#"
        //     SELECT id, cypher, nonce
        //     FROM bria_xpub_signer_configs
        //     WHERE id = ANY($1)
        //     "#,
        //     &ids
        // )
        // .fetch_all(&self.pool)
        // .await?;

        // let mut config_map: HashMap<Uuid, (ConfigCyper, Nonce)> = config_rows
        //     .into_iter()
        //     .map(|row| (row.id, (ConfigCyper(row.cypher), Nonce(row.nonce))))
        //     .collect();

        // let mut entity_events = HashMap::new();
        // for row in rows {
        //     let id = row.id;
        //     let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
        //     events.load_event(row.sequence as usize, row.event)?;
        // }

        // let mut xpubs = Vec::new();
        // for (id, events) in entity_events {
        //     let config = config_map.remove(&id);
        //     let xpub = Xpub::try_from((events, config))?;
        //     xpubs.push(xpub);
        // }

        // Ok(xpubs)
        !unimplemented!();
    }

    pub async fn list_all_xpubs(&self) -> Result<Vec<Xpub>, XpubError> {
        // let rows = sqlx::query!(
        //     r#"SELECT b.*, e.sequence, e.event
        //     FROM bria_xpubs b
        //     JOIN bria_xpub_events e ON b.id = e.id
        //     ORDER BY b.id, e.sequence"#,
        // )
        // .fetch_all(&self.pool)
        // .await?;
        // let config_rows = sqlx::query!(
        //     r#"
        //     SELECT id, cypher, nonce
        //     FROM bria_xpub_signer_configs
        //     "#,
        // )
        // .fetch_all(&self.pool)
        // .await?;

        // let mut config_map: HashMap<Uuid, (ConfigCyper, Nonce)> = config_rows
        //     .into_iter()
        //     .map(|row| (row.id, (ConfigCyper(row.cypher), Nonce(row.nonce))))
        //     .collect();

        // let mut entity_events = HashMap::new();
        // for row in rows {
        //     let id = row.id;
        //     let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
        //     events.load_event(row.sequence as usize, row.event)?;
        // }

        // let mut xpubs = Vec::new();
        // for (id, events) in entity_events {
        //     let config = config_map.remove(&id);
        //     let xpub = Xpub::try_from((events, config))?;
        //     xpubs.push(xpub);
        // }

        // Ok(xpubs)
        !unimplemented!();
    }
}
