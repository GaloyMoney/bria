use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::primitives::*;

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
    pub target_confs: u32,
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
            target_confs: 1,
            trigger: BatchGroupTrigger::Interval {
                seconds: default_interval(),
            },
        }
    }
}

fn default_interval() -> Duration {
    Duration::from_secs(300)
}
