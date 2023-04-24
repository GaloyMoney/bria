use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use std::collections::HashMap;

use super::entity::*;
use crate::{
    entity::*,
    error::*,
    primitives::{bitcoin::*, *},
};

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

        Self::persist_events(&mut tx, address).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn persist_if_not_present(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        address: NewAddress,
    ) -> Result<(), BriaError> {
        let res = sqlx::query!(
            r#"INSERT INTO bria_addresses
               (id, account_id, wallet_id, keychain_id, profile_id, address, address_idx, kind, external_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT DO NOTHING"#,
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
        .execute(&mut *tx)
        .await?;

        if res.rows_affected() == 0 {
            return Ok(());
        }
        Self::persist_events(tx, address).await
    }

    async fn persist_events(
        tx: &mut Transaction<'_, Postgres>,
        address: NewAddress,
    ) -> Result<(), BriaError> {
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
        query.execute(&mut *tx).await?;
        Ok(())
    }

    pub async fn find_external_by_wallet_id(
        &self,
        account_id: AccountId,
        wallet_id: WalletId,
    ) -> Result<Vec<WalletAddress>, BriaError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.id, b.address, b.external_id, e.sequence, e.event_type, e.event as "event?"
              FROM bria_addresses b
              JOIN bria_address_events e ON b.id = e.id
              WHERE account_id = $1 AND wallet_id = $2 AND kind = 'external'
              ORDER BY b.created_at, b.id, sequence"#,
            Uuid::from(account_id),
            Uuid::from(wallet_id)
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entity_events = HashMap::new();
        let mut ids = Vec::new();
        for mut row in rows {
            let id = AddressId::from(row.id);
            let address: bitcoin::Address = row.address.parse()?;
            ids.push((id, address));
            let sequence = row.sequence;
            let event = row.event.take().expect("Missing event");
            let events = entity_events
                .entry(id)
                .or_insert_with(EntityEvents::<AddressEvent>::new);
            events.load_event(sequence as usize, event)?;
        }
        let mut ret = Vec::new();
        for (id, address) in ids {
            if let Some(events) = entity_events.remove(&id) {
                ret.push(WalletAddress {
                    address,
                    wallet_id,
                    events,
                })
            }
        }
        Ok(ret)
    }
}
