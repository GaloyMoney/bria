use es_entity::*;
use sqlx::{Pool, Postgres};
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
        fingerprint(ty = "XPubId", create(accessor=id()), update(persist = false))
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

    pub async fn persist_updated(
        &self,
        op: &mut DbOp<'_>,
        mut xpub: Xpub,
    ) -> Result<(), XpubError> {
        if xpub.events.any_new() {
            self.persist_events(op, &mut xpub.events).await?;
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
                xpub.id as XpubId,
                cypher_bytes,
                nonce_bytes,
            )
            .execute(&mut **op.tx())
            .await?;
        }

        Ok(())
    }

    pub async fn find_from_ref(
        &self,
        account_id: AccountId,
        xpub_ref: impl Into<XPubRef>,
    ) -> Result<Xpub, XpubError> {
        let xpub_ref = xpub_ref.into();
        let mut xpub = match xpub_ref {
            XPubRef::Id(fp) => {
                let xpub = es_entity::es_query!(
                    "bria",
                    &self.pool,
                    r#"
                          SELECT *
                FROM bria_xpubs
                WHERE account_id = $1 AND fingerprint = $2"#,
                    Uuid::from(account_id),
                    fp.as_bytes()
                )
                .fetch_one()
                .await?;
                xpub
            }
            XPubRef::Name(name) => {
                let xpub = es_entity::es_query!(
                    "bria",
                    &self.pool,
                    r#"
                          SELECT *
                FROM bria_xpubs
                WHERE account_id = $1 AND name = $2"#,
                    Uuid::from(account_id),
                    name
                )
                .fetch_one()
                .await?;
                xpub
            }
        };

        let config_row = sqlx::query!(
            r#"
            SELECT cypher, nonce
            FROM bria_xpub_signer_configs
            WHERE id = $1
            "#,
            xpub.id as XpubId,
        )
        .fetch_optional(&self.pool)
        .await?;

        match config_row {
            Some(row) => {
                xpub.encrypted_signer_config = Some((ConfigCyper(row.cypher), Nonce(row.nonce)))
            }
            None => xpub.encrypted_signer_config = None,
        };

        Ok(xpub)
    }

    pub async fn list_xpubs(&self, account_id: AccountId) -> Result<Vec<Xpub>, XpubError> {
        let mut xpubs = vec![];
        let mut next = Some(PaginatedQueryArgs::default());
        while let Some(query) = next.take() {
            let mut paginated_xpub = self
                .list_for_account_id_by_id(account_id, query, es_entity::ListDirection::Ascending)
                .await?;

            xpubs.append(&mut paginated_xpub.entities);
            next = paginated_xpub.into_next_query();
        }

        let ids: Vec<Uuid> = xpubs.iter().map(|row| row.id.into()).collect();

        let config_rows = sqlx::query!(
            r#"
            SELECT id, cypher, nonce
            FROM bria_xpub_signer_configs
            WHERE id = ANY($1)
            ORDER BY id ASC
            "#,
            &ids
        )
        .fetch_all(&self.pool)
        .await?;

        let mut config_map: HashMap<Uuid, (ConfigCyper, Nonce)> = config_rows
            .into_iter()
            .map(|row| (row.id, (ConfigCyper(row.cypher), Nonce(row.nonce))))
            .collect();

        for xpub in &mut xpubs {
            if let Some(config) = config_map.remove(&xpub.id.into()) {
                xpub.encrypted_signer_config = Some(config);
            } else {
                xpub.encrypted_signer_config = None;
            }
        }

        Ok(xpubs)
    }

    pub async fn list_all_xpubs(&self) -> Result<Vec<Xpub>, XpubError> {
        let mut xpubs = vec![];
        let mut next = Some(PaginatedQueryArgs::default());
        while let Some(query) = next.take() {
            let mut paginated_xpub = self
                .list_by_id(query, es_entity::ListDirection::default())
                .await?;
            xpubs.append(&mut paginated_xpub.entities);
            next = paginated_xpub.into_next_query();
        }
        let config_rows = sqlx::query!(
            r#"
            SELECT id, cypher, nonce
            FROM bria_xpub_signer_configs
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut config_map: HashMap<XpubId, (ConfigCyper, Nonce)> = config_rows
            .into_iter()
            .map(|row| (row.id.into(), (ConfigCyper(row.cypher), Nonce(row.nonce))))
            .collect();
        for xpub in &mut xpubs {
            if let Some(config) = config_map.remove(&xpub.id) {
                xpub.encrypted_signer_config = Some(config);
            } else {
                xpub.encrypted_signer_config = None;
            }
        }

        Ok(xpubs)
    }
}
