use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::primitives::TxPriority;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchGroupConfig {
    pub tx_priority: TxPriority,
    pub consolidate_deprecated_keychains: bool,
    pub trigger: BatchGroupTrigger,
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BatchGroupTrigger {
    Manual,
    Immediate,
    Interval {
        #[serde_as(as = "serde_with::DurationSeconds<u64>")]
        #[serde(default = "default_interval")]
        seconds: Duration,
    },
}

impl Default for BatchGroupConfig {
    fn default() -> Self {
        Self {
            tx_priority: TxPriority::NextBlock,
            consolidate_deprecated_keychains: true,
            trigger: BatchGroupTrigger::Interval {
                seconds: default_interval(),
            },
        }
    }
}

fn default_interval() -> Duration {
    Duration::from_secs(60)
}
