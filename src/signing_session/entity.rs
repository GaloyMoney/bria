use derive_builder::Builder;
use es_entity::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use std::collections::HashMap;

use crate::{
    primitives::{bitcoin::psbt, *},
    xpub::SigningClientError,
};

#[derive(EsEvent, Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
#[es_event(id = "SigningSessionId")]
pub enum SigningSessionEvent {
    Initialized {
        id: SigningSessionId,
        xpub_id: XPubFingerprint,
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

#[derive(EsEntity, Builder)]
#[builder(pattern = "owned", build_fn(error = "EsEntityError"))]
pub struct SigningSession {
    pub id: SigningSessionId,
    pub account_id: AccountId,
    pub batch_id: BatchId,
    pub xpub_fingerprint: XPubFingerprint,
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
        for event in self.events.iter_all() {
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
        for event in self.events.iter_all() {
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
        for event in self.events.iter_all() {
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

#[derive(Error, Debug, Serialize, Deserialize, Clone)]
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
    pub xpub_sessions: HashMap<XPubFingerprint, SigningSession>,
}

#[derive(Builder, Clone, Debug)]
pub struct NewSigningSession {
    #[builder(private)]
    pub(super) id: SigningSessionId,
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
    pub(super) xpub_fingerprint: XPubFingerprint,
    unsigned_psbt: psbt::PartiallySignedTransaction,
}

impl NewSigningSession {
    pub fn builder() -> NewSigningSessionBuilder {
        let mut builder = NewSigningSessionBuilder::default();
        builder.id(SigningSessionId::new());
        builder
    }
}

impl IntoEvents<SigningSessionEvent> for NewSigningSession {
    fn into_events(self) -> EntityEvents<SigningSessionEvent> {
        let events = vec![SigningSessionEvent::Initialized {
            id: self.id,
            account_id: self.account_id,
            batch_id: self.batch_id,
            xpub_id: self.xpub_fingerprint,
            unsigned_psbt: self.unsigned_psbt,
        }];
        EntityEvents::init(self.id, events)
    }
}

impl TryFromEvents<SigningSessionEvent> for SigningSession {
    fn try_from_events(events: EntityEvents<SigningSessionEvent>) -> Result<Self, EsEntityError> {
        let mut builder = SigningSessionBuilder::default();
        for event in events.iter_all() {
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
                    .xpub_fingerprint(*xpub_id)
                    .unsigned_psbt(unsigned_psbt.clone());
            }
        }
        builder.events(events).build()
    }
}
