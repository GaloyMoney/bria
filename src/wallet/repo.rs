use sqlx::{Pool, Postgres};
use sqlx_ledger::AccountId as LedgerAccountId;
use uuid::Uuid;

use super::entity::*;
use crate::{error::*, primitives::*};

pub struct Wallets {
    pool: Pool<Postgres>,
}

impl Wallets {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create(
        &self,
        account_id: AccountId,
        ledger_account_id: LedgerAccountId,
        new_wallet: NewWallet,
    ) -> Result<WalletId, BriaError> {
        let tx = self.pool.begin().await?;
        let record = sqlx::query!(
            r#"INSERT INTO keychains (account_id, config)
            VALUES ((SELECT id FROM accounts WHERE id = $1), $2)
            RETURNING (id)"#,
            Uuid::from(account_id),
            serde_json::to_value(new_wallet.keychain)?
        )
        .fetch_one(&self.pool)
        .await?;
        let record = sqlx::query!(
            r#"INSERT INTO wallets (id, account_id, ledger_account_id, keychain_id, name)
            VALUES ($1, $2, (SELECT id FROM sqlx_ledger_accounts WHERE id = $3 LIMIT 1), $4, $5)
            RETURNING (id)"#,
            Uuid::from(new_wallet.id),
            Uuid::from(account_id),
            Uuid::from(ledger_account_id),
            record.id,
            new_wallet.name
        )
        .fetch_one(&self.pool)
        .await?;
        tx.commit().await?;
        Ok(WalletId::from(record.id))
    }
}
