use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::primitives::TxPriority;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PayoutQueueConfig {
    pub tx_priority: TxPriority,
    #[serde(default)]
    pub cpfp_payouts_after_mins: Option<u32>,
    #[serde(default)]
    pub cpfp_payouts_after_blocks: Option<u32>,
    pub consolidate_deprecated_keychains: bool,
    pub trigger: PayoutQueueTrigger,
}

impl PayoutQueueConfig {
    pub fn cpfp_payouts_detected_before(&self) -> chrono::DateTime<chrono::Utc> {
        let now = chrono::Utc::now();
        self.cpfp_payouts_after_mins
            .map(|mins| now - Duration::from_secs(mins as u64 * 60))
            .unwrap_or(now)
    }

    pub fn cpfp_payouts_detected_before_block(&self, current_height: u32) -> u32 {
        self.cpfp_payouts_after_blocks
            .map(|blocks| (current_height + 1).max(blocks) - blocks)
            .unwrap_or(current_height + 1)
    }

    pub fn should_cpfp(&self) -> bool {
        self.cpfp_payouts_after_mins.is_some() || self.cpfp_payouts_after_blocks.is_some()
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
            cpfp_payouts_after_blocks: None,
        }
    }
}

fn default_interval() -> Duration {
    Duration::from_secs(60)
}
