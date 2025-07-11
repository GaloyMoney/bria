use es_entity::*;
use sqlx::{Pool, Postgres};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::{entity::*, error::WalletError};
use crate::primitives::*;

#[derive(EsRepo, Clone, Debug)]
#[es_repo(
    entity = "Wallet",
    err = "WalletError",
    columns(
        name(ty = "String"),
        account_id(ty = "AccountId", list_for, update(persist = false))
    ),
    tbl_prefix = "bria"
)]
pub struct Wallets {
    pool: Pool<Postgres>,
}

impl Wallets {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn list_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Wallet>, WalletError> {
        let mut wallets = Vec::new();
        let mut next = Some(PaginatedQueryArgs::default());

        while let Some(query) = next.take() {
            let mut res = self
                .list_for_account_id_by_id(account_id, query, Default::default())
                .await?;
            wallets.append(&mut res.entities);
            next = res.into_next_query();
        }
        Ok(wallets)
    }

    pub async fn find_by_account_id_and_id(
        &self,
        account_id: AccountId,
        id: WalletId,
    ) -> Result<Wallet, WalletError> {
        let wallet = self.find_by_id(id).await?;
        if wallet.account_id != account_id {
            return Err(WalletError::EsEntityError(EsEntityError::NotFound));
        }
        Ok(wallet)
    }

    pub async fn find_by_account_id_and_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<Wallet, WalletError> {
        let wallet = es_entity::es_query!(
            "bria",
            &self.pool,
            r#"
            SELECT *
            FROM bria_wallets
            WHERE account_id = $1 and name = $2"#,
            account_id as AccountId,
            name as String,
        )
        .fetch_one()
        .await?;
        Ok(wallet)
    }

    pub async fn all_ids(
        &self,
    ) -> Result<impl Iterator<Item = (AccountId, WalletId)>, WalletError> {
        let rows =
            sqlx::query!(r#"SELECT DISTINCT account_id, id as wallet_id FROM bria_wallets"#,)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|row| {
            (
                AccountId::from(row.account_id),
                WalletId::from(row.wallet_id),
            )
        }))
    }
}
