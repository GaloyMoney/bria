use bitcoin::util::bip32::{ExtendedPubKey, Fingerprint};
use sqlx::{Pool, Postgres};
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

use super::value::*;
use crate::{error::*, primitives::*};

pub struct XPubs {
    pool: Pool<Postgres>,
}

impl XPubs {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    #[instrument(name = "xpubs.persist", skip(self))]
    pub async fn persist(
        &self,
        account_id: AccountId,
        key_name: String,
        xpub: XPub,
    ) -> Result<XPubId, BriaError> {
        let id = xpub.id();
        sqlx::query!(
            r#"INSERT INTO bria_xpubs
            (account_id, name, original, xpub, derivation_path, fingerprint, parent_fingerprint)
            VALUES ((SELECT id FROM bria_accounts WHERE id = $1), $2, $3, $4, $5, $6, $7)"#,
            Uuid::from(account_id),
            key_name,
            xpub.original,
            &xpub.inner.encode(),
            xpub.derivation.map(|d| d.to_string()),
            id.as_bytes(),
            xpub.inner.parent_fingerprint.as_bytes(),
        )
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn find_from_ref(
        &self,
        account_id: AccountId,
        xpub_ref: String,
    ) -> Result<(String, XPub), BriaError> {
        let (name, derivation_path, original, bytes) = match (
            Fingerprint::from_str(&xpub_ref),
            ExtendedPubKey::from_str(&xpub_ref),
        ) {
            (Ok(fp), _) => {
                let record = sqlx::query!(
                    r#"SELECT name, derivation_path, original, xpub FROM bria_xpubs WHERE account_id = $1 AND fingerprint = $2"#,
                    Uuid::from(account_id),
                    fp.as_bytes()
                )
                .fetch_one(&self.pool)
                .await?;
                (
                    record.name,
                    record.derivation_path,
                    record.original,
                    record.xpub,
                )
            }

            (_, Ok(key)) => {
                let record = sqlx::query!(
                    r#"SELECT name, derivation_path, original, xpub FROM bria_xpubs WHERE account_id = $1 AND xpub = $2"#,
                    Uuid::from(account_id),
                    &key.encode()
                )
                .fetch_one(&self.pool)
                .await?;
                (
                    record.name,
                    record.derivation_path,
                    record.original,
                    record.xpub,
                )
            }
            _ => {
                let record = sqlx::query!(
                    r#"SELECT name, derivation_path, original, xpub FROM bria_xpubs WHERE account_id = $1 AND name = $2"#,
                    Uuid::from(account_id),
                    xpub_ref
                )
                .fetch_one(&self.pool)
                .await?;
                (
                    record.name,
                    record.derivation_path,
                    record.original,
                    record.xpub,
                )
            }
        };
        Ok((
            name,
            XPub {
                derivation: derivation_path
                    .map(|d| d.parse().expect("Couldn't decode derivation path")),
                original,
                inner: ExtendedPubKey::decode(&bytes).expect("Couldn't decode xpub"),
            },
        ))
    }
}
