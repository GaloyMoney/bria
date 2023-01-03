use bitcoin::util::bip32::{ExtendedPubKey, Fingerprint};
use sqlx::{Pool, Postgres};
use std::str::FromStr;
use tracing::instrument;
use uuid::Uuid;

use super::{entity::*, signer::*, value::*};
use crate::{error::*, primitives::*};

pub struct XPubs {
    pool: Pool<Postgres>,
}

impl XPubs {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    #[instrument(name = "xpubs.persist", skip(self))]
    pub async fn persist(&self, xpub: NewXPub) -> Result<XPubId, BriaError> {
        let id = xpub.id();
        sqlx::query!(
            r#"INSERT INTO bria_xpubs
            (account_id, name, original, xpub, derivation_path, fingerprint, parent_fingerprint)
            VALUES ((SELECT id FROM bria_accounts WHERE id = $1), $2, $3, $4, $5, $6, $7)"#,
            Uuid::from(xpub.account_id),
            xpub.key_name,
            xpub.value.original,
            &xpub.value.inner.encode(),
            xpub.value.derivation.map(|d| d.to_string()),
            id.as_bytes(),
            xpub.value.inner.parent_fingerprint.as_bytes(),
        )
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn find_from_ref(
        &self,
        account_id: AccountId,
        xpub_ref: String,
    ) -> Result<AccountXPub, BriaError> {
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
        Ok(AccountXPub {
            account_id,
            key_name: name,
            value: XPub {
                derivation: derivation_path
                    .map(|d| d.parse().expect("Couldn't decode derivation path")),
                original,
                inner: ExtendedPubKey::decode(&bytes).expect("Couldn't decode xpub"),
            },
        })
    }

    #[instrument(name = "xpubs.set_signer_for_xpub", skip(self))]
    pub async fn set_signer_for_xpub(
        &self,
        account_id: AccountId,
        signer: NewSigner,
    ) -> Result<SignerId, BriaError> {
        sqlx::query!(
            r#"
            INSERT INTO bria_xpub_signers (id, account_id, xpub_name, signer_cfg)
            VALUES ($1, $2, (SELECT name FROM bria_xpubs WHERE account_id = $2 AND name = $3), $4)
            "#,
            Uuid::from(signer.id),
            Uuid::from(account_id),
            signer.xpub_name,
            serde_json::to_value(signer.config)?,
        )
        .execute(&self.pool)
        .await?;

        Ok(signer.id)
    }
}
