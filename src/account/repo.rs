use sqlx::{Pool, Postgres};
use sqlx_ledger::JournalId;
use uuid::Uuid;

use super::entity::*;
use crate::{admin::error::*, primitives::*};

pub struct Accounts {
    pool: Pool<Postgres>,
}

impl Accounts {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create(
        &self,
        name: String,
        journal_id: JournalId,
    ) -> Result<Account, AdminApiError> {
        let record = sqlx::query!(
            r#"INSERT INTO accounts (name, journal_id)
            VALUES ($1, (SELECT id FROM sqlx_ledger_journals WHERE id = $2 LIMIT 1))
            RETURNING (id)"#,
            name,
            Uuid::from(journal_id),
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(Account {
            name,
            id: AccountId::from(record.id),
        })
    }
}
