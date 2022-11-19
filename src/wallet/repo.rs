use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use super::entity::*;
use crate::{error::*, primitives::*};

pub struct Wallets {
    _pool: Pool<Postgres>,
}

impl Wallets {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self {
            _pool: pool.clone(),
        }
    }

    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: AccountId,
        new_wallet: NewWallet,
    ) -> Result<WalletId, BriaError> {
        let record = sqlx::query!(
            r#"INSERT INTO keychains (account_id, config)
            VALUES ($1, $2)
            RETURNING (id)"#,
            Uuid::from(account_id),
            serde_json::to_value(new_wallet.keychain)?
        )
        .fetch_one(&mut *tx)
        .await?;
        let record = sqlx::query!(
            r#"INSERT INTO wallets (id, account_id, ledger_account_id, keychain_id, name)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING (id)"#,
            Uuid::from(new_wallet.id),
            Uuid::from(account_id),
            Uuid::from(new_wallet.id),
            record.id,
            new_wallet.name
        )
        .fetch_one(&mut *tx)
        .await?;
        Ok(WalletId::from(record.id))
    }
}
