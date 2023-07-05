use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::collections::HashMap;

use crate::{
    entity::*,
    primitives::{bitcoin::psbt, *},
    xpub::SigningClientError,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SigningSessionEvent {
    Initialized {
        id: SigningSessionId,
        xpub_id: XPubId,
        account_id: AccountId,
        batch_id: BatchId,
        unsigned_psbt: psbt::PartiallySignedTransaction,
    },
    SigningAttemptFailed {
        reason: SigningFailureReason,
    },
    ExternallySignedPsbtSubmitted {
        signed_psbt: psbt::PartiallySignedTransaction,
    },
    RemoteSigningCompleted {
        signed_psbt: psbt::PartiallySignedTransaction,
    },
}

#[derive(Debug)]
pub enum SigningSessionState {
    Initialized,
    Failed,
    Complete,
}

#[derive(Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct SigningSession {
    pub id: SigningSessionId,
    pub account_id: AccountId,
    pub batch_id: BatchId,
    pub xpub_id: XPubId,
    pub unsigned_psbt: psbt::PartiallySignedTransaction,
    pub(super) events: EntityEvents<SigningSessionEvent>,
}

impl SigningSession {
    pub fn attempt_failed(&mut self, reason: impl Into<SigningFailureReason>) {
        self.events.push(SigningSessionEvent::SigningAttemptFailed {
            reason: reason.into(),
        });
    }

    pub fn remote_signing_complete(&mut self, signed_psbt: psbt::PartiallySignedTransaction) {
        self.events
            .push(SigningSessionEvent::RemoteSigningCompleted { signed_psbt })
    }

    pub fn submit_externally_signed_psbt(&mut self, signed_psbt: psbt::PartiallySignedTransaction) {
        self.events
            .push(SigningSessionEvent::ExternallySignedPsbtSubmitted { signed_psbt })
    }

    pub fn is_completed(&self) -> bool {
        self.signed_psbt().is_some()
    }

    pub fn signed_psbt(&self) -> Option<&psbt::PartiallySignedTransaction> {
        let mut ret = None;
        for event in self.events.iter() {
            match event {
                SigningSessionEvent::RemoteSigningCompleted { signed_psbt }
                | SigningSessionEvent::ExternallySignedPsbtSubmitted { signed_psbt } => {
                    ret = Some(signed_psbt);
                }
                _ => (),
            }
        }
        ret
    }

    pub fn failure_reason(&self) -> Option<&SigningFailureReason> {
        let mut ret = None;
        for event in self.events.iter() {
            if let SigningSessionEvent::SigningAttemptFailed { reason } = event {
                ret = Some(reason);
            }
            ret = match event {
                SigningSessionEvent::SigningAttemptFailed { reason } => Some(reason),
                SigningSessionEvent::RemoteSigningCompleted { .. } => None,
                SigningSessionEvent::ExternallySignedPsbtSubmitted { .. } => None,
                _ => ret,
            };
        }
        ret
    }

    pub fn state(&self) -> SigningSessionState {
        let mut ret = SigningSessionState::Initialized;
        for event in self.events.iter() {
            ret = match event {
                SigningSessionEvent::SigningAttemptFailed { .. } => SigningSessionState::Failed,
                SigningSessionEvent::RemoteSigningCompleted { .. } => SigningSessionState::Complete,
                SigningSessionEvent::ExternallySignedPsbtSubmitted { .. } => {
                    SigningSessionState::Complete
                }
                _ => ret,
            };
        }
        ret
    }
}

#[derive(Error, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SigningFailureReason {
    #[error("SignerConfigMissing")]
    SignerConfigMissing,
    #[error("{err}")]
    SigningClientError { err: String },
}

impl From<&SigningClientError> for SigningFailureReason {
    fn from(err: &SigningClientError) -> Self {
        Self::SigningClientError {
            err: err.to_string(),
        }
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
    pub(super) xpub_id: XPubId,
    unsigned_psbt: psbt::PartiallySignedTransaction,
}

impl NewSigningSession {
    pub fn builder() -> NewSigningSessionBuilder {
        let mut builder = NewSigningSessionBuilder::default();
        builder.id(SigningSessionId::new());
        builder
    }

    pub(super) fn initial_events(self) -> EntityEvents<SigningSessionEvent> {
        EntityEvents::init([SigningSessionEvent::Initialized {
            id: self.id,
            account_id: self.account_id,
            batch_id: self.batch_id,
            xpub_id: self.xpub_id,
            unsigned_psbt: self.unsigned_psbt,
        }])
    }
}

impl TryFrom<EntityEvents<SigningSessionEvent>> for SigningSession {
    type Error = EntityError;

    fn try_from(events: EntityEvents<SigningSessionEvent>) -> Result<Self, Self::Error> {
        let mut builder = SigningSessionBuilder::default();
        for event in events.iter() {
            if let SigningSessionEvent::Initialized {
                id,
                account_id,
                batch_id,
                unsigned_psbt,
                xpub_id,
            } = event
            {
                builder = builder
                    .id(*id)
                    .account_id(*account_id)
                    .batch_id(*batch_id)
                    .xpub_id(*xpub_id)
                    .unsigned_psbt(unsigned_psbt.clone());
            }
        }
        builder.events(events).build()
    }
}
