mod config;
mod server;

use super::{app::*, error::*};
pub use config::*;
pub use server::*;

pub async fn run(
    pool: sqlx::PgPool,
    config: ApiConfig,
    blockchain_cfg: BlockchainConfig,
    app_cfg: AppConfig,
) -> Result<(), BriaError> {
    let app = App::run(pool, blockchain_cfg, app_cfg).await?;
    server::start(config, app).await?;
    Ok(())
}
