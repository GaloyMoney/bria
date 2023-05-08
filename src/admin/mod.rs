mod app;
mod config;
pub mod error;
mod keys;
mod server;

use crate::primitives::bitcoin;

pub use app::*;
pub use config::*;
pub use error::*;
use keys::*;
pub use server::*;

pub async fn run(
    pool: sqlx::PgPool,
    config: AdminApiConfig,
    network: bitcoin::Network,
) -> Result<(), AdminApiError> {
    let app = AdminApp::new(pool, network);
    server::start(config, app).await?;
    Ok(())
}
