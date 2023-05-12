use super::db::DbConfig;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{
    admin::AdminApiConfig, api::ApiConfig, app::*, dev_constants, primitives::bitcoin,
    tracing::TracingConfig, xpub::EncryptionKey,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub db: DbConfig,
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
    pub signer_encryption_key: String,
}

impl Config {
    pub fn from_path(
        path: impl AsRef<Path>,
        EnvOverride {
            db_con,
            signer_encryption_key,
        }: EnvOverride,
    ) -> anyhow::Result<Self> {
        let config_file = std::fs::read_to_string(path).context("Couldn't read config file")?;
        let mut config: Config =
            serde_yaml::from_str(&config_file).context("Couldn't parse config file")?;
        config.db.pg_con = db_con;

        match (
            hex::decode(signer_encryption_key),
            config.app.blockchain.network,
        ) {
            (Ok(key_bytes), _) => {
                config.app.signer_encryption.key =
                    EncryptionKey::clone_from_slice(key_bytes.as_ref())
            }
            (_, network) if network != bitcoin::Network::Bitcoin => {
                config.app.signer_encryption.key = dev_constants::dev_signer_encryption_key();
            }
            (Err(e), _) => {
                return Err(e.into());
            }
        }
        Ok(config)
    }
}
