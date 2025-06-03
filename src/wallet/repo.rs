use es_entity::*;
use sqlx::{Pool,Postgres,Transaction};
use uuid::Uuid;
use std::collections::{HashMap, HashSet};
use super::{entity::*, error::WalletError};
use crate::{dev_constants, primitives::*};
use serde_json::Value;

#[derive(EsRepo, Clone, Debug)]
#[es_repo(
    entity = "Wallet",
    err = "WalletError",
    columns(
        name(ty = "String"),
        account_id(ty = "AccountId", list_for)
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

    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        new_wallet: NewWallet,
    ) -> Result<WalletId, WalletError> {
        let record = sqlx::query!(
            r#"INSERT INTO bria_wallets (id, account_id, name)
               VALUES ($1, $2, $3)
               RETURNING id"#,
            Uuid::from(new_wallet.id),
            Uuid::from(new_wallet.account_id),
            new_wallet.name
        )
        .fetch_one(&mut **tx)
        .await?;

        let mut events = new_wallet.initial_events();
        for (i, event) in events.iter_all().enumerate() {
            let event_json = serde_json::to_value(event).map_err(WalletError::SerdeJson)?;
            sqlx::query!(
                r#"INSERT INTO bria_wallet_events (id, sequence, event)
                    VALUES ($1, $2, $3)"#,
                Uuid::from(record.id),
                i as i64,
                event_json
            )
            .execute(&mut **tx)
            .await?;
        }

        Ok(WalletId::from(record.id))
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
        let wallet = self.find_by_name(name).await?;
        if wallet.account_id != account_id {
            return Err(WalletError::EsEntityError(EsEntityError::NotFound));
        }
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
    pub async fn find_by_ids(
    &self,
    ids: HashSet<WalletId>,
) -> Result<HashMap<WalletId, Wallet>, WalletError> {
    let uuids = ids.into_iter().map(Uuid::from).collect::<Vec<_>>();

    let rows = sqlx::query!(
        r#"
          SELECT b.id, e.sequence, e.event
          FROM bria_wallets b
          JOIN bria_wallet_events e ON b.id = e.id
          WHERE b.id = ANY($1)
          ORDER BY b.id, e.sequence"#,
        &uuids[..]
    )
    .fetch_all(&self.pool)
    .await?;

    let mut event_map: HashMap<WalletId, Vec<WalletEvent>> = HashMap::new();
    for row in rows {
        let id = WalletId::from(row.id);
        let event: WalletEvent = serde_json::from_value(row.event)
            .map_err(WalletError::SerdeJson)?;
        event_map.entry(id).or_default().push(event);
    }

    let mut wallets = HashMap::new();
    for (id, events) in event_map {
        let events = EntityEvents::init(id, events);
        wallets.insert(id, Wallet::try_from_events(events)?);
    }
    Ok(wallets)
}
}
