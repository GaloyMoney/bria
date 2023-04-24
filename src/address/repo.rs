use sqlx::{Pool, Postgres};
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

    pub async fn persist_address(&self, address: NewAddress) -> Result<(), BriaError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            r#"INSERT INTO bria_addresses
               (id, account_id, wallet_id, keychain_id, profile_id, address, address_idx, kind, external_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
            Uuid::from(address.id),
            Uuid::from(address.account_id),
            Uuid::from(address.wallet_id),
            Uuid::from(address.keychain_id),
            address.profile_id.map(Uuid::from),
            address.address.to_string(),
            address.address_idx as i32,
            pg::PgKeychainKind::from(address.kind) as pg::PgKeychainKind,
            address.external_id,
        )
        .execute(&mut tx)
        .await?;

        let mut query_builder = sqlx::QueryBuilder::new(
            r#"INSERT INTO bria_address_events
            (id, sequence, event_type, event)"#,
        );
        let id = address.id;
        query_builder.push_values(
            address.initial_events().new_serialized_events(id),
            |mut builder, (id, sequence, event_type, event)| {
                builder.push_bind(id);
                builder.push_bind(sequence);
                builder.push_bind(event_type);
                builder.push_bind(event);
            },
        );
        let query = query_builder.build();
        query.execute(&mut tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
