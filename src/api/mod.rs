mod config;
mod server;

use super::{app::*, error::*};
pub use config::*;
pub use server::*;

pub async fn run(
    pool: sqlx::PgPool,
    config: ApiConfig,
    migrate_on_start: bool,
    blockchain_cfg: BlockchainConfig,
    wallets_cfg: WalletsConfig,
) -> Result<(), BriaError> {
    let app = App::run(pool, migrate_on_start, blockchain_cfg, wallets_cfg).await?;
    server::start(config, app).await?;
    Ok(())
}
