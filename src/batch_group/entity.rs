use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::config::*;
use crate::{entity::*, primitives::*};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BatchGroupEvent {
    Initialized {
        id: BatchGroupId,
        account_id: AccountId,
    },
    NameUpdated {
        name: String,
    },
    DescriptionUpdated {
        description: String,
    },
    ConfigUpdated {
        config: BatchGroupConfig,
    },
}

#[derive(Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct BatchGroup {
    pub id: BatchGroupId,
    pub account_id: AccountId,
    pub name: String,
    pub config: BatchGroupConfig,

    pub(super) events: EntityEvents<BatchGroupEvent>,
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

    pub fn description(&self) -> Option<String> {
        let mut ret = None;
        for event in self.events.iter() {
            if let BatchGroupEvent::DescriptionUpdated { description } = event {
                ret = Some(description.as_str());
            }
        }
        ret.map(|s| s.to_string())
    }

    pub fn update_description(&mut self, description: String) {
        self.events
            .push(BatchGroupEvent::BatchGroupDescriptionUpdated { description });
    }
}

#[derive(Debug, Builder, Clone)]
pub struct NewBatchGroup {
    #[builder(setter(into))]
    pub(super) id: BatchGroupId,
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) name: String,
    #[builder(default)]
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

    pub(super) fn initial_events(self) -> EntityEvents<BatchGroupEvent> {
        let mut events = EntityEvents::init([
            BatchGroupEvent::Initialized {
                id: self.id,
                account_id: self.account_id,
            },
            BatchGroupEvent::NameUpdated { name: self.name },
            BatchGroupEvent::ConfigUpdated {
                config: self.config,
            },
        ]);
        if let Some(description) = self.description {
            events.push(BatchGroupEvent::DescriptionUpdated { description });
        }
        events
    }
}

impl TryFrom<EntityEvents<BatchGroupEvent>> for BatchGroup {
    type Error = EntityError;

    fn try_from(events: EntityEvents<BatchGroupEvent>) -> Result<Self, Self::Error> {
        let mut builder = BatchGroupBuilder::default();
        use BatchGroupEvent::*;
        for event in events.iter() {
            match event {
                Initialized { id, account_id } => {
                    builder = builder.id(*id).account_id(*account_id);
                }
                NameUpdated { name } => {
                    builder = builder.name(name.clone());
                }
                ConfigUpdated { config } => {
                    builder = builder.config(config.clone());
                }
                _ => (),
            }
        }
        builder.events(events).build()
    }
}
