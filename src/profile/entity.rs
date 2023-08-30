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
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub id: ProfileId,
    pub account_id: AccountId,
    pub name: String,
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
}

impl NewProfile {
    pub fn builder() -> NewProfileBuilder {
        let mut builder = NewProfileBuilder::default();
        builder.id(ProfileId::new());
        builder
    }

    pub(super) fn initial_events(self) -> EntityEvents<ProfileEvent> {
        EntityEvents::init([
            ProfileEvent::Initialized {
                id: self.id,
                account_id: self.account_id,
            },
            ProfileEvent::NameUpdated { name: self.name },
        ])
    }
}
