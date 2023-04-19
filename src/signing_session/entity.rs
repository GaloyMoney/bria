use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use crate::{entity::*, primitives::*, xpub::*};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SigningSessionEvent {
    SigningSessionInitialized,
}

pub struct SigningSession {
    pub id: SigningSessionId,
    pub account_id: AccountId,
    pub batch_id: BatchId,
    pub xpub_id: XPubId,
    pub(super) events: EntityEvents<SigningSessionEvent>,
}

pub struct BatchSigningSession {
    pub xpub_sessions: HashMap<XPubId, SigningSession>,
}

#[derive(Builder, Clone, Debug)]
pub struct NewSigningSession {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
    pub(super) xpub: XPub,
    #[builder(private)]
    pub(super) events: Vec<SigningSessionEvent>,
}

impl NewSigningSession {
    pub fn builder() -> NewSigningSessionBuilder {
        let mut builder = NewSigningSessionBuilder::default();
        builder.events(vec![SigningSessionEvent::SigningSessionInitialized]);
        builder
    }
}
