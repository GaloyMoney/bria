use es_entity::*;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use super::{entity::*, error::AddressError};

use crate::primitives::{bitcoin::*, *};

#[derive(EsRepo, Clone)]
#[es_repo(
    entity = "WalletAddress",
    event = "AddressEvent",
    err = "AddressError",
    tbl = "bria_addresses",
    id = Uuid,
    events_tbl = "bria_address_events",
    columns(
        wallet_id(ty = "WalletId", update(persist = false)),
        account_id(ty = "AccountId", update(persist = false)),
        keychain_id(ty = "KeychainId", update(persist = false)),
        profile_id(ty = "Option<ProfileId>", update(persist = false)),
        address(ty = "String", create(accessor = "address.to_string()"), update(persist = false)),
        kind(
            ty = "pg::PgKeychainKind",
            create(accessor = "kind.into()"),
            update(persist = false)
        ),
        external_id(ty = "String")
    ),
    tbl_prefix = "bria"
)]
pub struct Addresses {
    pool: Pool<Postgres>,
}

impl Addresses {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn persist_if_not_present(
        &self,
        op: &mut DbOp<'_>,
        address: NewWalletAddress,
    ) -> Result<(), AddressError> {
        let res = sqlx::query!(
            r#"INSERT INTO bria_addresses
               (id, account_id, wallet_id, keychain_id, profile_id, address, kind, external_id)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT DO NOTHING"#,
            address.id,
            address.account_id as AccountId,
            address.wallet_id as WalletId,
            address.keychain_id as KeychainId,
            address.profile_id as Option<ProfileId>,
            address.address.to_string(),
            pg::PgKeychainKind::from(address.kind) as pg::PgKeychainKind,
            address.external_id,
        )
        .execute(&mut **op.tx())
        .await?;

        if res.rows_affected() == 0 {
            return Ok(());
        }
        self.persist_events(op, &mut address.into_events()).await?;
        Ok(())
    }

    pub async fn list_external_by_wallet_id(
        &self,
        account_id: AccountId,
        wallet_id: WalletId,
    ) -> Result<Vec<WalletAddress>, AddressError> {
        let mut wallet_addresses = Vec::new();
        let mut query = es_entity::PaginatedQueryArgs::<
            wallet_address_cursor::WalletAddressesByCreatedAtCursor,
        > {
            first: Default::default(),
            after: None,
        };
        loop {
            let (id, created_at) = if let Some(after) = query.after {
                (Some(after.id), Some(after.created_at))
            } else {
                (None, None)
            };

            let (entities, has_next_page) = es_entity::es_query!(
                entity_ty = WalletAddress,
                id_ty = Uuid,
                "bria",
                &self.pool,
                r#"
                SELECT *
                FROM bria_addresses
                WHERE account_id = $1 AND wallet_id = $2 AND kind = 'external'
                AND (COALESCE((created_at, id) > ($4, $3), $3 IS NULL))
                ORDER BY created_at, id"#,
                account_id as AccountId,
                wallet_id as WalletId,
                id,
                created_at
            )
            .fetch_n(query.first)
            .await?;

            wallet_addresses.extend(entities);

            if !has_next_page {
                break;
            }
            let end_cursor = wallet_addresses
                .last()
                .map(wallet_address_cursor::WalletAddressesByCreatedAtCursor::from);

            query.after = end_cursor;
        }
        Ok(wallet_addresses)
    }

    pub async fn find_by_account_id_and_address(
        &self,
        account_id: AccountId,
        address: String,
    ) -> Result<WalletAddress, AddressError> {
        let wallet_address = es_entity::es_query!(
            entity_ty = WalletAddress,
            id_ty = Uuid,
            "bria",
            &self.pool,
            r#"
            SELECT *
            FROM bria_addresses
            WHERE account_id = $1 AND address = $2"#,
            account_id as AccountId,
            address.to_string()
        )
        .fetch_one()
        .await?;
        Ok(wallet_address)
    }

    pub async fn find_by_account_id_and_external_id(
        &self,
        account_id: AccountId,
        external_id: String,
    ) -> Result<WalletAddress, AddressError> {
        let wallet_address = es_entity::es_query!(
            entity_ty = WalletAddress,
            id_ty = Uuid,
            "bria",
            &self.pool,
            r#"
            SELECT *
            FROM bria_addresses
            WHERE account_id = $1 AND external_id = $2"#,
            account_id as AccountId,
            external_id
        )
        .fetch_one()
        .await?;
        Ok(wallet_address)
    }
}
