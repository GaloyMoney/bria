use serde::{Deserialize, Serialize};
use std::time::Duration;

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(default)]
    pub signing: SigningJobConfig,
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningJobConfig {
    #[serde(default = "default_signing_warn_retries")]
    pub warn_retries: u32,
    #[serde(default = "default_signing_max_attempts")]
    pub max_attempts: u32,
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(default = "default_signing_max_retry_delay")]
    pub max_retry_delay: Duration,
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            sync_all_wallets_delay: default_sync_all_wallets_delay(),
            process_all_payout_queues_delay: default_process_all_payout_queues_delay(),
            respawn_all_outbox_handlers_delay: default_respawn_all_outbox_handlers_delay(),
            signing: SigningJobConfig::default(),
        }
    }
}

impl Default for SigningJobConfig {
    fn default() -> Self {
        Self {
            warn_retries: default_signing_warn_retries(),
            max_attempts: default_signing_max_attempts(),
            max_retry_delay: default_signing_max_retry_delay(),
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

fn default_signing_warn_retries() -> u32 {
    9 // About 8 minutes
}

fn default_signing_max_attempts() -> u32 {
    25 // About 90 minutes
}

fn default_signing_max_retry_delay() -> Duration {
    Duration::from_secs(300)
}
