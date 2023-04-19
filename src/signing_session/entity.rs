use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use crate::{entity::*, primitives::*};

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
    pub unsigned_psbt: bitcoin::psbt::PartiallySignedTransaction,
    pub(super) _events: EntityEvents<SigningSessionEvent>,
}

pub struct BatchSigningSession {
    pub xpub_sessions: HashMap<XPubId, SigningSession>,
}

#[derive(Builder, Clone, Debug)]
pub struct NewSigningSession {
    #[builder(private)]
    pub(super) id: SigningSessionId,
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
    pub(super) unsigned_psbt: bitcoin::psbt::PartiallySignedTransaction,
    #[builder(private)]
    pub(super) events: EntityEvents<SigningSessionEvent>,
}

impl NewSigningSession {
    pub fn builder() -> NewSigningSessionBuilder {
        let mut builder = NewSigningSessionBuilder::default();
        builder.id(SigningSessionId::new());
        builder.events(EntityEvents::init(
            SigningSessionEvent::SigningSessionInitialized,
        ));
        builder
    }
}
