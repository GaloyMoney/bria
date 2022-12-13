use rand::distributions::{Alphanumeric, DistString};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use super::entity::*;
use crate::{error::*, primitives::*};

pub struct AccountApiKeys {
    pool: Pool<Postgres>,
}

impl AccountApiKeys {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        name: String,
        account_id: AccountId,
    ) -> Result<AccountApiKey, BriaError> {
        let code = Alphanumeric.sample_string(&mut rand::thread_rng(), 64);
        let key = format!("bria_{}", code);
        let record = sqlx::query!(
            r#"INSERT INTO bria_account_api_keys (name, encrypted_key, account_id)
            VALUES ($1, crypt($2, gen_salt('bf')), (SELECT id FROM bria_accounts WHERE id = $3)) RETURNING (id)"#,
            name,
            key,
            Uuid::from(account_id),
        )
        .fetch_one(&mut *tx)
        .await?;
        Ok(AccountApiKey {
            name,
            key,
            id: AccountApiKeyId::from(record.id),
            account_id,
        })
    }

    pub async fn find_by_key(&self, key: &str) -> Result<AccountApiKey, BriaError> {
        let record = sqlx::query!(
            r#"SELECT id, account_id, name FROM bria_account_api_keys WHERE encrypted_key = crypt($1, encrypted_key)"#,
            key
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(AccountApiKey {
            name: record.name,
            account_id: AccountId::from(record.account_id),
            key: key.to_string(),
            id: AccountApiKeyId::from(record.id),
        })
    }
}
