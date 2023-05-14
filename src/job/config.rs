use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde_with::serde_as]
pub struct JobsConfig {
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

impl Default for JobsConfig {
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
