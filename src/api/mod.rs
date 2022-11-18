mod config;
mod server;

use super::{app::App, error::*};
pub use config::*;
pub use server::*;

pub async fn run(pool: sqlx::PgPool, config: ApiConfig) -> Result<(), BriaError> {
    let app = App::new(pool);
    server::start(config, app).await?;
    Ok(())
}
