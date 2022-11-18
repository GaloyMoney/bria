use rand::distributions::{Alphanumeric, DistString};
use sqlx::{Pool, Postgres};

use super::entity::*;
use crate::{admin::error::*, primitives::*};

pub(in crate::admin) struct AdminApiKeys {
    pool: Pool<Postgres>,
}

impl AdminApiKeys {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create(&self, name: String) -> Result<AdminApiKey, AdminApiError> {
        let code = Alphanumeric.sample_string(&mut rand::thread_rng(), 64);
        let key = format!("bria_admin_{}", code);
        let record = sqlx::query!(
            r#"INSERT INTO admin_api_keys (name, encrypted_key)
            VALUES ($1, crypt($2, gen_salt('bf'))) RETURNING (id)"#,
            name,
            key
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(AdminApiKey {
            name,
            key,
            id: AdminApiKeyId::from(record.id),
        })
    }

    pub async fn find_by_key(&self, key: &str) -> Result<AdminApiKey, AdminApiError> {
        let record = sqlx::query!(
            r#"SELECT id, name FROM admin_api_keys WHERE encrypted_key = crypt($1, encrypted_key)"#,
            key
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(AdminApiKey {
            name: record.name,
            key: key.to_string(),
            id: AdminApiKeyId::from(record.id),
        })
    }
}
