use sqlx::{Pool, Postgres};

use super::entity::*;
use crate::{admin::error::*, primitives::*};

pub struct Accounts {
    pool: Pool<Postgres>,
}

impl Accounts {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create(&self, name: String) -> Result<Account, AdminApiError> {
        let record = sqlx::query!(
            r#"INSERT INTO accounts (name)
            VALUES ($1) RETURNING (id)"#,
            name,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(Account {
            name,
            id: AccountId::from(record.id),
        })
    }
}
