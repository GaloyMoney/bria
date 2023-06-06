mod config;
mod server;

use crate::app::{error::*, *};

pub use config::*;
pub use server::*;

pub async fn run(
    pool: sqlx::PgPool,
    config: ApiConfig,
    app_cfg: AppConfig,
) -> Result<(), ApplicationError> {
    let app = App::run(pool, app_cfg).await?;
    server::start(config, app).await?;
    Ok(())
}
