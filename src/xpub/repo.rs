use bitcoin::util::bip32::{ExtendedPubKey, Fingerprint};
use sqlx::{Pool, Postgres};
use std::str::FromStr;
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

    pub async fn persist(
        &self,
        account_id: AccountId,
        name: String,
        xpub: XPub,
    ) -> Result<XPubId, BriaError> {
        let id = xpub.id();
        sqlx::query!(
            r#"INSERT INTO xpubs (account_id, name, original, xpub, fingerprint)
            VALUES ((SELECT id FROM accounts WHERE id = $1), $2, $3, $4, $5)"#,
            Uuid::from(account_id),
            name,
            xpub.original,
            &xpub.inner.encode(),
            id.as_bytes(),
        )
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn find_from_ref(
        &self,
        account_id: AccountId,
        xpub_ref: String,
    ) -> Result<XPub, BriaError> {
        let (original, bytes) = match (
            Fingerprint::from_str(&xpub_ref),
            ExtendedPubKey::from_str(&xpub_ref),
        ) {
            (Ok(fp), _) => {
                let record = sqlx::query!(
                    r#"SELECT original, xpub FROM xpubs WHERE account_id = $1 AND fingerprint = $2"#,
                    Uuid::from(account_id),
                    fp.as_bytes()
                )
                .fetch_one(&self.pool)
                .await?;
                (record.original, record.xpub)
            }

            (_, Ok(key)) => {
                let record = sqlx::query!(
                    r#"SELECT original, xpub FROM xpubs WHERE account_id = $1 AND xpub = $2"#,
                    Uuid::from(account_id),
                    &key.encode()
                )
                .fetch_one(&self.pool)
                .await?;
                (record.original, record.xpub)
            }
            _ => {
                let record = sqlx::query!(
                    r#"SELECT original, xpub FROM xpubs WHERE account_id = $1 AND name = $2"#,
                    Uuid::from(account_id),
                    xpub_ref
                )
                .fetch_one(&self.pool)
                .await?;
                (record.original, record.xpub)
            }
        };
        Ok(XPub {
            original,
            inner: ExtendedPubKey::decode(&bytes).expect("Couldn't decode xpub"),
        })
    }
}
