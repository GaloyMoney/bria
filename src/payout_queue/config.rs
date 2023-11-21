use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::primitives::TxPriority;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayoutQueueConfig {
    pub tx_priority: TxPriority,
    #[serde(default)]
    pub cpfp_payouts_after_mins: Option<u32>,
    pub consolidate_deprecated_keychains: bool,
    pub trigger: PayoutQueueTrigger,
}

impl PayoutQueueConfig {
    pub fn cpfp_payouts_after(&self) -> Option<Duration> {
        self.cpfp_payouts_after_mins
            .map(|mins| Duration::from_secs(mins as u64 * 60))
    }
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PayoutQueueTrigger {
    Interval {
        #[serde_as(as = "serde_with::DurationSeconds<u64>")]
        #[serde(default = "default_interval")]
        seconds: Duration,
    },
    Manual,
}

impl Default for PayoutQueueConfig {
    fn default() -> Self {
        Self {
            tx_priority: TxPriority::NextBlock,
            consolidate_deprecated_keychains: false,
            trigger: PayoutQueueTrigger::Interval {
                seconds: default_interval(),
            },
            cpfp_payouts_after_mins: None,
        }
    }
}

fn default_interval() -> Duration {
    Duration::from_secs(60)
}
