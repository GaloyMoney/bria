use super::{entity::*, error::AddressError};
use crate::primitives::{bitcoin::*, *};
use es_entity::*;
use sqlx::{Database, Encode, Pool, Postgres, Transaction};
use uuid::Uuid;

#[derive(EsRepo, Clone)]
#[es_repo(
    entity = "WalletAddress",
    err = "AddressError",
    tbl = "bria_addresses",
    id = Uuid,
    events_tbl = "bria_address_events",
    columns(
        wallet_id(ty = "WalletId", update(persist = false)),
        account_id(ty = "AccountId", update(persist = false)),
        keychain_id(ty = "KeychainId", update(persist = false)),
        profile_id(ty = "Option<ProfileId>", update(persist = false)),
        address(ty = "Address", update(persist = false)),
        kind(ty = "pg::PgKeychainKind", update(persist = false)),
        external_id(ty = "String")
    ),
    tbl_prefix = "bria"
)]
pub struct Addresses {
    pool: Pool<Postgres>,
}
impl Encode<'_, Postgres> for Address {
    fn encode_by_ref(
        &self,
        buf: &mut <Postgres as Database>::ArgumentBuffer<'_>,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let bytes = self.to_string();
        <String as Encode<'_, Postgres>>::encode_by_ref(&bytes, buf)
    }
}
impl sqlx::Type<Postgres> for Address {
    fn type_info() -> <Postgres as Database>::TypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("VARCHAR")
    }
}
impl Addresses {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    // pub async fn persist_new_address(&self, address: NewAddress) -> Result<(), AddressError> {
    //     let mut tx = self.pool.begin().await?;
    //     sqlx::query!(
    //         r#"INSERT INTO bria_addresses
    //            (id, account_id, wallet_id, keychain_id, profile_id, address, kind, external_id)
    //            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    //         address.db_uuid,
    //         address.account_id as AccountId,
    //         address.wallet_id as WalletId,
    //         address.keychain_id as KeychainId,
    //         address.profile_id as Option<ProfileId>,
    //         address.address.to_string(),
    //         pg::PgKeychainKind::from(address.kind) as pg::PgKeychainKind,
    //         address.external_id,
    //     )
    //     .execute(&mut *tx)
    //     .await?;

    //     Self::persist_events(&mut tx, address).await?;
    //     tx.commit().await?;
    //     Ok(())
    // }

    pub async fn persist_if_not_present(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        address: NewWalletAddress,
    ) -> Result<(), AddressError> {
        // let res = sqlx::query!(
        //     r#"INSERT INTO bria_addresses
        //        (id, account_id, wallet_id, keychain_id, profile_id, address, kind, external_id)
        //        VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT DO NOTHING"#,
        //     address.db_uuid,
        //     address.account_id as AccountId,
        //     address.wallet_id as WalletId,
        //     address.keychain_id as KeychainId,
        //     address.profile_id as Option<ProfileId>,
        //     address.address.to_string(),
        //     pg::PgKeychainKind::from(address.kind) as pg::PgKeychainKind,
        //     address.external_id,
        // )
        // .execute(&mut **tx)
        // .await?;

        // if res.rows_affected() == 0 {
        //     return Ok(());
        // }
        // Self::persist_events(tx, address).await
        unimplemented!();
    }

    // async fn persist_events(
    //     tx: &mut Transaction<'_, Postgres>,
    //     address: NewAddress,
    // ) -> Result<(), AddressError> {
    //     let id = address.db_uuid;
    //     EntityEvents::<AddressEvent>::persist(
    //         "bria_address_events",
    //         tx,
    //         address.initial_events().new_serialized_events(id),
    //     )
    //     .await?;
    //     Ok(())
    // }

    pub async fn list_external_by_wallet_id(
        &self,
        account_id: AccountId,
        wallet_id: WalletId,
    ) -> Result<Vec<WalletAddress>, AddressError> {
        // let mut wallet_addresses = Vec::new();
        // let mut query = es_entity::PaginatedQueryArgs::<
        //     wallet_address_cursor::WalletAddressesByCreatedAtCursor,
        // > {
        //     first: Default::default(),
        //     after: None,
        // };
        // loop {
        //     let es_entity::PaginatedQueryArgs { first, after } = query;
        //     let (id, created_at) = if let Some(after) = after {
        //         (Some(after.id), Some(after.created_at))
        //     } else {
        //         (None, None)
        //     };

        //     let (entities, has_next_page) = es_entity::es_query!(
        //         "bria",
        //         &self.pool,
        //         r#"
        //         SELECT *
        //         FROM bria_addresses
        //         WHERE account_id = $1 AND wallet_id = $2 AND kind = 'external'
        //         AND (COALESCE((created_at, id) > ($4, $3), $3 IS NULL))
        //         ORDER BY created_at, id"#,
        //         account_id as AccountId,
        //         wallet_id as WalletId,
        //         id as Option<Uuid>,
        //         created_at as Option<chrono::DateTime<chrono::Utc>>,
        //     )
        //     .fetch_n(first)
        //     .await?;

        //     wallet_addresses.extend(entities);

        //     if !has_next_page {
        //         break;
        //     }
        //     let end_cursor = wallet_addresses
        //         .last()
        //         .map(wallet_address_cursor::WalletAddressesByCreatedAtCursor::from);

        //     query.after = end_cursor;
        // }
        // Ok(wallet_addresses)
        unimplemented!();

        // let rows = sqlx::query!(
        //     r#"
        //       SELECT b.id, e.sequence, e.event
        //       FROM bria_addresses b
        //       JOIN bria_address_events e ON b.id = e.id
        //       WHERE account_id = $1 AND wallet_id = $2 AND kind = 'external'
        //       ORDER BY b.created_at, b.id, sequence"#,
        //     account_id as AccountId,
        //     wallet_id as WalletId
        // )
        // .fetch_all(&self.pool)
        // .await?;
        // let mut entity_events = HashMap::new();
        // let mut ids = Vec::new();
        // for row in rows {
        //     let id = row.id;
        //     ids.push(id);
        //     let events = entity_events
        //         .entry(id)
        //         .or_insert_with(EntityEvents::<AddressEvent>::new);
        //     events.load_event(row.sequence as usize, row.event)?;
        // }
        // let mut ret = Vec::new();
        // for id in ids {
        //     if let Some(events) = entity_events.remove(&id) {
        //         ret.push(WalletAddress::try_from(events)?);
        //     }
        // }
        // Ok(ret)
    }

    pub async fn find_by_account_id_and_address(
        &self,
        account_id: AccountId,
        address: String,
    ) -> Result<WalletAddress, AddressError> {
        unimplemented!();
        // let wallet_address = es_entity::es_query!(
        //     id_ty = Uuid,
        //     "bria",
        //     &self.pool,
        //     r#"
        //     SELECT *
        //     FROM bria_addresses
        //     WHERE account_id = $1 AND address = $2"#,
        //     Uuid::from(account_id),
        //     address
        // )
        // .fetch_one()
        // .await?;
        // Ok(wallet_address)
        // let rows = sqlx::query!(
        //     r#"
        //       SELECT b.id, e.sequence, e.event
        //       FROM bria_addresses b
        //       JOIN bria_address_events e ON b.id = e.id
        //       WHERE account_id = $1 AND address = $2
        //       ORDER BY b.created_at, b.id, sequence"#,
        //     account_id as AccountId,
        //     address
        // )
        // .fetch_all(&self.pool)
        // .await?;

        // if rows.is_empty() {
        //     return Err(AddressError::AddressNotFound(address));
        // }

        // let mut events = EntityEvents::new();
        // for row in rows {
        //     events.load_event(row.sequence as usize, row.event)?;
        // }
        // Ok(WalletAddress::try_from(events)?)
    }

    pub async fn find_by_account_id_and_external_id(
        &self,
        account_id: AccountId,
        external_id: String,
    ) -> Result<WalletAddress, AddressError> {
        unimplemented!();
        // let wallet_address = es_entity::es_query!(
        //     id_ty = Uuid,
        //     "bria",
        //     &self.pool,
        //     r#"
        //     SELECT *
        //     FROM bria_addresses
        //     WHERE account_id = $1 AND external_id = $2"#,
        //     Uuid::from(account_id),
        //     external_id
        // )
        // .fetch_one()
        // .await?;
        // Ok(wallet_address)
        // let rows = sqlx::query!(
        //     r#"
        //       SELECT b.id, e.sequence, e.event
        //       FROM bria_addresses b
        //       JOIN bria_address_events e ON b.id = e.id
        //       WHERE account_id = $1 AND external_id = $2
        //       ORDER BY b.created_at, b.id, sequence"#,
        //     account_id as AccountId,
        //     external_id
        // )
        // .fetch_all(&self.pool)
        // .await?;
        // if rows.is_empty() {
        //     return Err(AddressError::ExternalIdNotFound);
        // }
        // let mut events = EntityEvents::new();
        // for row in rows {
        //     events.load_event(row.sequence as usize, row.event)?;
        // }
        // Ok(WalletAddress::try_from(events)?)
        // unimplemented!();
    }
}
