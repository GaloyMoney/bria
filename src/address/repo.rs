use sqlx::{Pool, Postgres, Transaction};

use std::collections::HashMap;

use super::{entity::*, error::AddressError};
use crate::{
    entity::*,
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

    pub async fn persist_new_address(&self, address: NewAddress) -> Result<(), AddressError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            r#"INSERT INTO bria_addresses
               (id, account_id, wallet_id, keychain_id, profile_id, address, kind, external_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            address.db_uuid,
            address.account_id as AccountId,
            address.wallet_id as WalletId,
            address.keychain_id as KeychainId,
            address.profile_id as Option<ProfileId>,
            address.address.to_string(),
            pg::PgKeychainKind::from(address.kind) as pg::PgKeychainKind,
            address.external_id,
        )
        .execute(&mut *tx)
        .await?;

        Self::persist_events(&mut tx, address).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn persist_if_not_present(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        address: NewAddress,
    ) -> Result<(), AddressError> {
        let res = sqlx::query!(
            r#"INSERT INTO bria_addresses
               (id, account_id, wallet_id, keychain_id, profile_id, address, kind, external_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT DO NOTHING"#,
            address.db_uuid,
            address.account_id as AccountId,
            address.wallet_id as WalletId,
            address.keychain_id as KeychainId,
            address.profile_id as Option<ProfileId>,
            address.address.to_string(),
            pg::PgKeychainKind::from(address.kind) as pg::PgKeychainKind,
            address.external_id,
        )
        .execute(&mut **tx)
        .await?;

        if res.rows_affected() == 0 {
            return Ok(());
        }
        Self::persist_events(tx, address).await
    }

    pub async fn update(&self, address: WalletAddress) -> Result<(), AddressError> {
        if !address.events.is_dirty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            r#"UPDATE bria_addresses
               SET external_id = $1
               WHERE account_id = $2 AND address = $3"#,
            address.external_id,
            address.account_id as AccountId,
            address.address.to_string()
        )
        .execute(&mut *tx)
        .await?;
        EntityEvents::<AddressEvent>::persist(
            "bria_address_events",
            &mut tx,
            address.events.new_serialized_events(address.db_uuid),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn persist_events(
        tx: &mut Transaction<'_, Postgres>,
        address: NewAddress,
    ) -> Result<(), AddressError> {
        let id = address.db_uuid;
        EntityEvents::<AddressEvent>::persist(
            "bria_address_events",
            tx,
            address.initial_events().new_serialized_events(id),
        )
        .await?;
        Ok(())
    }

    pub async fn list_external_by_wallet_id(
        &self,
        account_id: AccountId,
        wallet_id: WalletId,
    ) -> Result<Vec<WalletAddress>, AddressError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.id, e.sequence, e.event
              FROM bria_addresses b
              JOIN bria_address_events e ON b.id = e.id
              WHERE account_id = $1 AND wallet_id = $2 AND kind = 'external'
              ORDER BY b.created_at, b.id, sequence"#,
            account_id as AccountId,
            wallet_id as WalletId
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entity_events = HashMap::new();
        let mut ids = Vec::new();
        for row in rows {
            let id = row.id;
            ids.push(id);
            let events = entity_events
                .entry(id)
                .or_insert_with(EntityEvents::<AddressEvent>::new);
            events.load_event(row.sequence as usize, row.event)?;
        }
        let mut ret = Vec::new();
        for id in ids {
            if let Some(events) = entity_events.remove(&id) {
                ret.push(WalletAddress::try_from(events)?);
            }
        }
        Ok(ret)
    }

    pub async fn find_by_address(
        &self,
        account_id: AccountId,
        address: String,
    ) -> Result<WalletAddress, AddressError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.id, e.sequence, e.event
              FROM bria_addresses b
              JOIN bria_address_events e ON b.id = e.id
              WHERE account_id = $1 AND address = $2
              ORDER BY b.created_at, b.id, sequence"#,
            account_id as AccountId,
            address
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Err(AddressError::AddressNotFound(address));
        }

        let mut events = EntityEvents::new();
        for row in rows {
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(WalletAddress::try_from(events)?)
    }

    pub async fn find_by_external_id(
        &self,
        account_id: AccountId,
        external_id: String,
    ) -> Result<WalletAddress, AddressError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.id, e.sequence, e.event
              FROM bria_addresses b
              JOIN bria_address_events e ON b.id = e.id
              WHERE account_id = $1 AND external_id = $2
              ORDER BY b.created_at, b.id, sequence"#,
            account_id as AccountId,
            external_id
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Err(AddressError::ExternalIdNotFound);
        }
        let mut events = EntityEvents::new();
        for row in rows {
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(WalletAddress::try_from(events)?)
    }
}
