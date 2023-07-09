use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::config::*;
use crate::{entity::*, primitives::*};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PayoutQueueEvent {
    Initialized {
        id: PayoutQueueId,
        account_id: AccountId,
    },
    NameUpdated {
        name: String,
    },
    DescriptionUpdated {
        description: String,
    },
    ConfigUpdated {
        config: PayoutQueueConfig,
    },
}

#[derive(Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct PayoutQueue {
    pub id: PayoutQueueId,
    pub account_id: AccountId,
    pub name: String,
    pub config: PayoutQueueConfig,

    pub(super) events: EntityEvents<PayoutQueueEvent>,
}

impl PayoutQueue {
    pub fn spawn_in(&self) -> Option<Duration> {
        use PayoutQueueTrigger::*;
        match self.config.trigger {
            Interval { seconds } => Some(seconds),
            Manual => None,
        }
    }

    pub fn description(&self) -> Option<String> {
        let mut ret = None;
        for event in self.events.iter() {
            if let PayoutQueueEvent::DescriptionUpdated { description } = event {
                ret = Some(description.as_str());
            }
        }
        ret.map(|s| s.to_string())
    }

    pub fn update_description(&mut self, description: String) {
        if self.description().as_ref() != Some(&description) {
            self.events
                .push(PayoutQueueEvent::DescriptionUpdated { description });
        }
    }

    pub fn update_config(&mut self, config: PayoutQueueConfig) {
        if self.config != config {
            self.events.push(PayoutQueueEvent::ConfigUpdated { config });
        }
    }
}

#[derive(Debug, Builder, Clone)]
pub struct NewPayoutQueue {
    #[builder(setter(into))]
    pub(super) id: PayoutQueueId,
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) name: String,
    #[builder(default)]
    pub(super) description: Option<String>,
    #[builder(default)]
    pub(super) config: PayoutQueueConfig,
}

impl NewPayoutQueue {
    pub fn builder() -> NewPayoutQueueBuilder {
        let mut builder = NewPayoutQueueBuilder::default();
        builder.id(PayoutQueueId::new());
        builder
    }

    pub(super) fn initial_events(self) -> EntityEvents<PayoutQueueEvent> {
        let mut events = EntityEvents::init([
            PayoutQueueEvent::Initialized {
                id: self.id,
                account_id: self.account_id,
            },
            PayoutQueueEvent::NameUpdated { name: self.name },
            PayoutQueueEvent::ConfigUpdated {
                config: self.config,
            },
        ]);
        if let Some(description) = self.description {
            events.push(PayoutQueueEvent::DescriptionUpdated { description });
        }
        events
    }
}

impl TryFrom<EntityEvents<PayoutQueueEvent>> for PayoutQueue {
    type Error = EntityError;

    fn try_from(events: EntityEvents<PayoutQueueEvent>) -> Result<Self, Self::Error> {
        let mut builder = PayoutQueueBuilder::default();
        use PayoutQueueEvent::*;
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
