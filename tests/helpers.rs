#![allow(dead_code)]

use bria::{admin::*, primitives::*};
use rand::distributions::{Alphanumeric, DistString};

pub async fn init_pool() -> anyhow::Result<sqlx::PgPool> {
    let pg_host = std::env::var("PG_HOST").unwrap_or("localhost".to_string());
    let pg_con = format!("postgres://user:password@{pg_host}:5432/pg");
    let pool = sqlx::PgPool::connect(&pg_con).await?;
    Ok(pool)
}

pub async fn create_test_account(pool: &sqlx::PgPool) -> anyhow::Result<AccountId> {
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let app = AdminApp::new(pool.clone());
    Ok(app.create_account(name).await?.account_id)
}

// pub async fn create_test_wallet(pool: &sqlx::pgPool) -> anyhow::Result<WalletId> {}
