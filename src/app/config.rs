use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::primitives::bitcoin::Network;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    #[serde(default = "default_network")]
    pub network: Network,
    #[serde(default = "default_electrum_url")]
    pub electrum_url: String,
}

impl Default for BlockchainConfig {
    fn default() -> Self {
        Self {
            network: default_network(),
            electrum_url: default_electrum_url(),
        }
    }
}

fn default_network() -> Network {
    Network::Regtest
}

fn default_electrum_url() -> String {
    "127.0.0.1:50001".to_string()
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(default = "default_sync_all_wallets_delay")]
    pub sync_all_wallets_delay: Duration,
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(default = "default_process_all_payout_queues_delay")]
    pub process_all_payout_queues_delay: Duration,
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(default = "default_respawn_all_outbox_handlers_delay")]
    pub respawn_all_outbox_handlers_delay: Duration,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sync_all_wallets_delay: default_sync_all_wallets_delay(),
            process_all_payout_queues_delay: default_process_all_payout_queues_delay(),
            respawn_all_outbox_handlers_delay: default_respawn_all_outbox_handlers_delay(),
        }
    }
}

fn default_sync_all_wallets_delay() -> Duration {
    Duration::from_secs(5)
}

fn default_process_all_payout_queues_delay() -> Duration {
    Duration::from_secs(2)
}

fn default_respawn_all_outbox_handlers_delay() -> Duration {
    Duration::from_secs(5)
}
