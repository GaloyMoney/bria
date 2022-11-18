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

    pub async fn create(
        &self,
        name: String,
        account_id: AccountId,
    ) -> Result<AccountApiKey, BriaError> {
        let code = Alphanumeric.sample_string(&mut rand::thread_rng(), 64);
        let key = format!("bria_{}", code);
        let record = sqlx::query!(
            r#"INSERT INTO account_api_keys (name, encrypted_key, account_id)
            VALUES ($1, crypt($2, gen_salt('bf')), $3) RETURNING (id)"#,
            name,
            key,
            Uuid::from(account_id),
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(AccountApiKey {
            name,
            key,
            id: AccountApiKeyId::from(record.id),
            account_id,
        })
    }
}
