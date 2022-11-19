use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use super::entity::*;
use crate::{admin::error::*, primitives::*};

pub struct Accounts {
    _pool: Pool<Postgres>,
}

impl Accounts {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self {
            _pool: pool.clone(),
        }
    }

    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        name: String,
    ) -> Result<Account, AdminApiError> {
        let id = Uuid::new_v4();
        let record = sqlx::query!(
            r#"INSERT INTO accounts (id, name, journal_id)
            VALUES ($1, $2, $1)
            RETURNING (id)"#,
            id,
            name,
        )
        .fetch_one(&mut *tx)
        .await?;
        Ok(Account {
            name,
            id: AccountId::from(record.id),
        })
    }
}
