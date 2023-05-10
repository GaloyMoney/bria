use super::db::DbConfig;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{admin::AdminApiConfig, api::ApiConfig, app::*, tracing::TracingConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub db: DbConfig,
    #[serde(default)]
    pub blockchain: BlockchainConfig,
    #[serde(default)]
    pub app: AppConfig,
    #[serde(default)]
    pub tracing: TracingConfig,
    #[serde(default)]
    pub admin: AdminApiConfig,
    #[serde(default)]
    pub api: ApiConfig,
}

pub struct EnvOverride {
    pub db_con: String,
    pub key: String,
    pub nonce: String,
}

impl Config {
    pub fn from_path(
        path: impl AsRef<Path>,
        EnvOverride { db_con, key, nonce }: EnvOverride,
    ) -> anyhow::Result<Self> {
        let config_file = std::fs::read_to_string(path).context("Couldn't read config file")?;
        let mut config: Config =
            serde_yaml::from_str(&config_file).context("Couldn't parse config file")?;

        config.db.pg_con = db_con;
        config.app.key = hex::decode(key)?;
        config.app.nonce = hex::decode(nonce)?;
        Ok(config)
    }
}
