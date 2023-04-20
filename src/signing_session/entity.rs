use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use std::{collections::HashMap, fmt};

use crate::{entity::*, primitives::*};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SigningSessionEvent {
    SigningSessionInitialized,
    SigningAttemptFailed { reason: SigningFailureReason },
}

#[derive(Debug)]
pub enum SigningSessionState {
    Initialized,
    Failed,
    Complete,
}

pub struct SigningSession {
    pub id: SigningSessionId,
    pub account_id: AccountId,
    pub batch_id: BatchId,
    pub xpub_id: XPubId,
    pub unsigned_psbt: bitcoin::psbt::PartiallySignedTransaction,
    pub(super) events: EntityEvents<SigningSessionEvent>,
}

impl SigningSession {
    pub fn signer_config_missing(&mut self) -> SigningFailureReason {
        self.events.push(SigningSessionEvent::SigningAttemptFailed {
            reason: SigningFailureReason::SignerConfigMissing,
        });
        SigningFailureReason::SignerConfigMissing
    }

    pub fn failure_reason(&self) -> Option<SigningFailureReason> {
        let mut ret = None;
        for event in self.events.iter() {
            if let SigningSessionEvent::SigningAttemptFailed { reason } = event {
                ret = Some(*reason);
            }
        }
        ret
    }

    pub fn state(&self) -> SigningSessionState {
        let mut ret = SigningSessionState::Initialized;
        for event in self.events.iter() {
            if let SigningSessionEvent::SigningAttemptFailed { .. } = event {
                ret = SigningSessionState::Failed
            }
        }
        ret
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SigningFailureReason {
    SignerConfigMissing,
}

impl fmt::Display for SigningFailureReason {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let value = serde_json::to_value(self).expect("Could not serialize SigningFailureReason");
        write!(f, "{}", value.as_str().expect("Could not convert to str"))
    }
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
