use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use super::{entity::*, error::AccountError};
use crate::{admin::error::*, primitives::*};

pub struct Accounts {
    pool: Pool<Postgres>,
}

impl Accounts {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_name: String,
    ) -> Result<Account, AdminApiError> {
        let id = Uuid::new_v4();
        let record = sqlx::query!(
            r#"INSERT INTO bria_accounts (id, name, journal_id)
            VALUES ($1, $2, $1)
            RETURNING (id)"#,
            id,
            account_name,
        )
        .fetch_one(&mut *tx)
        .await?;
        Ok(Account {
            name: account_name,
            id: AccountId::from(record.id),
        })
    }

    pub async fn list(&self) -> Result<Vec<Account>, AccountError> {
        let records = sqlx::query!(r#"SELECT id, name FROM bria_accounts"#)
            .fetch_all(&self.pool)
            .await?;

        let accounts = records
            .into_iter()
            .map(|record| Account {
                id: AccountId::from(record.id),
                name: record.name,
            })
            .collect();

        Ok(accounts)
    }
}
