use bitcoin::Network;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    #[serde(default = "default_network")]
    pub network: Network,
}

impl Default for BlockchainConfig {
    fn default() -> Self {
        Self {
            network: Network::Regtest,
        }
    }
}

fn default_network() -> Network {
    Network::Regtest
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletsConfig {
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(default = "default_sync_all_delay")]
    pub sync_all_delay: Duration,
}

impl Default for WalletsConfig {
    fn default() -> Self {
        Self {
            sync_all_delay: default_sync_all_delay(),
        }
    }
}

fn default_sync_all_delay() -> Duration {
    Duration::from_secs(10)
}
