use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use super::entity::*;
use crate::{error::*, primitives::bitcoin::*};

#[derive(Clone)]
pub struct Addresses {
    pool: Pool<Postgres>,
}

impl Addresses {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn persist_address(
        &self,
        address: NewAddress,
    ) -> Result<Transaction<'_, Postgres>, BriaError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            r#"INSERT INTO bria_addresses
               (id, account_id, wallet_id, keychain_id, profile_id, address,
               kind, address_index, external_id, metadata)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
            Uuid::from(address.id),
            Uuid::from(address.account_id),
            Uuid::from(address.wallet_id),
            Uuid::from(address.keychain_id),
            address.profile_id.map(Uuid::from),
            address.address,
            pg::PgKeychainKind::from(address.kind) as pg::PgKeychainKind,
            address.address_idx as i32,
            address.external_id,
            address.metadata,
        )
        .execute(&mut *tx)
        .await?;

        Ok(tx)
    }
}
