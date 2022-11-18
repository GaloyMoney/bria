mod app;
mod config;
pub mod error;
mod keys;
mod server;

use app::*;
pub use config::*;
pub use error::*;
use keys::*;
pub use server::*;

pub async fn run(pool: sqlx::PgPool, config: AdminApiConfig) -> Result<(), AdminApiError> {
    let app = AdminApp::new(pool);
    server::start(config, app).await?;
    Ok(())
}
