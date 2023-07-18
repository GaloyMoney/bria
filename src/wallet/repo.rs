use sqlx::{Pool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::{entity::*, error::*};
use crate::{entity::*, primitives::*};

#[derive(Debug, Clone)]
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
            r#"INSERT INTO bria_wallets (id, account_id, name) VALUES ($1, $2, $3) RETURNING (id)"#,
            Uuid::from(new_wallet.id),
            Uuid::from(new_wallet.account_id),
            new_wallet.name
        )
        .fetch_one(&mut **tx)
        .await?;
        EntityEvents::<WalletEvent>::persist(
            "bria_wallet_events",
            tx,
            new_wallet.initial_events().new_serialized_events(record.id),
        )
        .await?;
        Ok(WalletId::from(record.id))
    }

    pub async fn find_by_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<Wallet, WalletError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_wallets b
              JOIN bria_wallet_events e ON b.id = e.id
              WHERE account_id = $1 AND name = $2
              ORDER BY e.sequence"#,
            account_id as AccountId,
            name
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Err(WalletError::WalletNameNotFound(name));
        }
        let mut events = EntityEvents::new();
        for row in rows {
            events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(Wallet::try_from(events)?)
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

    pub async fn find_by_id(&self, id: WalletId) -> Result<Wallet, WalletError> {
        let ids: HashSet<WalletId> = std::iter::once(id).collect();
        if let Some(wallet) = self.find_by_ids(ids).await?.remove(&id) {
            Ok(wallet)
        } else {
            Err(WalletError::WalletIdNotFound(id.to_string()))
        }
    }

    pub async fn list_by_account_id(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Wallet>, WalletError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_wallets b
              JOIN bria_wallet_events e ON b.id = e.id
              WHERE account_id = $1
              ORDER BY e.sequence"#,
            account_id as AccountId,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut events = HashMap::new();
        for row in rows {
            let id = WalletId::from(row.id);
            let sequence = row.sequence;
            let events = events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(sequence as usize, row.event)?;
        }
        Ok(events
            .into_values()
            .map(Wallet::try_from)
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub async fn find_by_ids(
        &self,
        ids: HashSet<WalletId>,
    ) -> Result<HashMap<WalletId, Wallet>, WalletError> {
        let uuids = ids.into_iter().map(Uuid::from).collect::<Vec<_>>();
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_wallets b
              JOIN bria_wallet_events e ON b.id = e.id
              WHERE b.id = ANY($1)
              ORDER BY b.id, e.sequence"#,
            &uuids[..]
        )
        .fetch_all(&self.pool)
        .await?;
        let mut events = HashMap::new();
        for row in rows {
            let id = WalletId::from(row.id);
            let sequence = row.sequence;
            let events = events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(sequence as usize, row.event)?;
        }
        let mut wallets = HashMap::new();
        for (id, events) in events {
            wallets.insert(id, Wallet::try_from(events)?);
        }
        Ok(wallets)
    }
}
