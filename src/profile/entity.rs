use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::{entity::*, primitives::*};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProfileEvent {
    Initialized {
        id: ProfileId,
        account_id: AccountId,
    },
    NameUpdated {
        name: String,
    },
    SpendingPolicyUpdated {
        spending_policy: SpendingPolicy,
    },
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct Profile {
    pub id: ProfileId,
    pub account_id: AccountId,
    pub name: String,
    #[builder(default, setter(strip_option))]
    pub spending_policy: Option<SpendingPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingPolicy {
    pub allowed_destinations: Vec<bitcoin::Address>,
}

impl SpendingPolicy {
    pub fn is_destination_allowed(&self, address: &bitcoin::Address) -> bool {
        self.allowed_destinations.contains(address)
    }
}

pub struct ProfileApiKey {
    pub key: String,
    pub id: ProfileApiKeyId,
    pub profile_id: ProfileId,
    pub account_id: AccountId,
}

#[derive(Builder, Clone, Debug)]
pub struct NewProfile {
    #[builder(setter(into))]
    pub(super) id: ProfileId,
    #[builder(setter(into))]
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) name: String,
    #[builder(default)]
    pub(super) spending_policy: Option<SpendingPolicy>,
}

impl NewProfile {
    pub fn builder() -> NewProfileBuilder {
        let mut builder = NewProfileBuilder::default();
        builder.id(ProfileId::new());
        builder
    }

    pub(super) fn initial_events(self) -> EntityEvents<ProfileEvent> {
        let mut events = EntityEvents::init([
            ProfileEvent::Initialized {
                id: self.id,
                account_id: self.account_id,
            },
            ProfileEvent::NameUpdated { name: self.name },
        ]);
        if self.spending_policy.is_some() {
            events.push(ProfileEvent::SpendingPolicyUpdated {
                spending_policy: self.spending_policy.unwrap(),
            });
        }
        events
    }
}

impl TryFrom<EntityEvents<ProfileEvent>> for Profile {
    type Error = EntityError;

    fn try_from(events: EntityEvents<ProfileEvent>) -> Result<Self, Self::Error> {
        let mut builder = ProfileBuilder::default();
        for event in events.into_iter() {
            match event {
                ProfileEvent::Initialized { id, account_id } => {
                    builder = builder.id(id).account_id(account_id)
                }
                ProfileEvent::NameUpdated { name } => {
                    builder = builder.name(name);
                }
                ProfileEvent::SpendingPolicyUpdated { spending_policy } => {
                    builder = builder.spending_policy(spending_policy);
                }
            }
        }
        builder.build()
    }
}
