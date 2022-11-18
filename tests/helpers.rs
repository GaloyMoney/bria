use bria::{admin::*, primitives::*};
use rand::distributions::{Alphanumeric, DistString};

pub async fn create_test_account(pool: &sqlx::PgPool) -> anyhow::Result<AccountId> {
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let app = AdminApp::new(pool.clone());
    Ok(app.create_account(name).await?.account_id)
}
