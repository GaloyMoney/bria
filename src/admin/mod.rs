mod app;
mod config;
pub mod error;
mod keys;
mod server;

use crate::{dev_constants, job_svc::JobSvc, primitives::bitcoin, token_store};

pub use app::*;
pub use config::*;
pub use error::*;
use keys::*;
pub use server::*;

pub async fn run_dev(
    pool: sqlx::PgPool,
    config: AdminApiConfig,
    network: bitcoin::Network,
    bria_home: String,
    job_svc: JobSvc,
) -> Result<(), AdminApiError> {
    let app = AdminApp::new(pool, network, job_svc);
    let (admin_key, profile_key) = app.dev_bootstrap().await?;
    token_store::store_admin_token(&bria_home, &admin_key.key)?;
    println!("Admin API key");
    println!(
        "---\nname: {}\nkey: {}\nid: {}",
        admin_key.name, admin_key.key, admin_key.id,
    );
    token_store::store_profile_token(&bria_home, &profile_key.key)?;

    println!("New Account");
    println!(
        "---\nname: {}\nid: {}\nkey: {}\nprofile_id: {}",
        dev_constants::DEV_ACCOUNT_NAME,
        profile_key.account_id,
        profile_key.key,
        profile_key.profile_id,
    );
    server::start(config, app).await?;
    Ok(())
}

pub async fn run(
    pool: sqlx::PgPool,
    config: AdminApiConfig,
    network: bitcoin::Network,
    job_svc: JobSvc,
) -> Result<(), AdminApiError> {
    let app = AdminApp::new(pool, network, job_svc);
    server::start(config, app).await?;
    Ok(())
}
