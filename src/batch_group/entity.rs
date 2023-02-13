use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::primitives::*;

pub struct BatchGroup {
    pub id: BatchGroupId,
    pub account_id: AccountId,
    pub name: String,
    pub config: BatchGroupConfig,
}

impl BatchGroup {
    pub fn spawn_in(&self) -> Option<Duration> {
        use BatchGroupTrigger::*;
        match self.config.trigger {
            Manual => None,
            Immediate => Some(Duration::from_secs(1)),
            Interval { seconds } => Some(seconds),
        }
    }
}

#[derive(Debug, Builder, Clone)]
pub struct NewBatchGroup {
    #[builder(setter(into))]
    pub(super) id: BatchGroupId,
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) name: String,
    #[builder(default, setter(into, strip_option))]
    pub(super) description: Option<String>,
    #[builder(default)]
    pub(super) config: BatchGroupConfig,
}

impl NewBatchGroup {
    pub fn builder() -> NewBatchGroupBuilder {
        let mut builder = NewBatchGroupBuilder::default();
        builder.id(BatchGroupId::new());
        builder
    }
}

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
    Duration::from_secs(20)
}
