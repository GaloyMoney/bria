use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    #[serde(default)]
    pub pg_con: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    #[serde(default = "bool_true")]
    pub migrate_on_start: bool,
}

pub async fn init_pool(config: &DbConfig) -> anyhow::Result<sqlx::PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.pool_size)
        .connect(&config.pg_con)
        .await?;
    if config.migrate_on_start {
        sqlx::migrate!().run(&pool).await?;
    }
    Ok(pool)
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            pg_con: "".to_string(),
            pool_size: default_pool_size(),
            migrate_on_start: true,
        }
    }
}

fn default_pool_size() -> u32 {
    20
}

fn bool_true() -> bool {
    true
}
