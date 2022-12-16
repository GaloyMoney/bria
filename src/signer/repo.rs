use sqlx::{Pool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::entity::*;
use crate::{error::*, primitives::*};

#[derive(Debug, Clone)]
pub struct Signers {
    pool: Pool<Postgres>,
}

impl Signers {
    pub fn new(pool: &Pool<Postgres>, network: bitcoin::Network) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: AccountId,
        signer: NewSigner,
    ) -> Result<SignerId, BriaError> {
        sqlx::query!(
            r#"
            INSERT INTO bria_signers (id, account_id, xpub_name, signer_cfg)
            VALUES ($1, $2, (SELECT name FROM bria_xpubs WHERE account_id = $2 AND name = $3), $4)
            "#,
            Uuid::from(signer.id),
            Uuid::from(account_id),
            signer.xpub_name,
            serde_json::to_value(signer.config)?,
        )
        .execute(&mut *tx)
        .await?;

        Ok(signer.id)
    }
}
