use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::{entity::*, primitives::*};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PayoutEvent {
    PayoutInitialized {
        id: PayoutId,
        wallet_id: WalletId,
        batch_group_id: BatchGroupId,
        profile_id: ProfileId,
        destination: PayoutDestination,
        satoshis: Satoshis,
    },
    PayoutExternalIdUpdated {
        external_id: String,
    },
    PayoutMetadataUpdated {
        metadata: serde_json::Value,
    },
    PayoutAddedToBatch {
        batch_id: BatchId,
    },
}

#[derive(Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct Payout {
    pub id: PayoutId,
    pub wallet_id: WalletId,
    pub profile_id: ProfileId,
    pub batch_group_id: BatchGroupId,
    #[builder(setter(into), default)]
    pub batch_id: Option<BatchId>,
    pub satoshis: Satoshis,
    pub destination: PayoutDestination,
    pub external_id: String,
    #[builder(setter(into), default)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct UnbatchedPayout {
    pub id: PayoutId,
    pub wallet_id: WalletId,
    pub destination: PayoutDestination,
    pub satoshis: Satoshis,

    pub(super) events: EntityEvents<PayoutEvent>,
}

impl UnbatchedPayout {
    pub(super) fn add_to_batch(&mut self, batch_id: BatchId) {
        self.events
            .push(PayoutEvent::PayoutAddedToBatch { batch_id });
    }
}

#[derive(Debug, Builder, Clone)]
pub struct NewPayout {
    #[builder(setter(into))]
    pub(super) id: PayoutId,
    #[builder(setter(into))]
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) wallet_id: WalletId,
    #[builder(setter(into))]
    pub(super) batch_group_id: BatchGroupId,
    pub(super) profile_id: ProfileId,
    pub(super) satoshis: Satoshis,
    pub(super) destination: PayoutDestination,
    #[builder(setter(into))]
    pub(super) external_id: String,
    #[builder(default, setter(into))]
    pub(super) metadata: Option<serde_json::Value>,
}

impl NewPayout {
    pub fn builder() -> NewPayoutBuilder {
        let mut builder = NewPayoutBuilder::default();
        let id = PayoutId::new();
        builder.external_id(id.to_string()).id(id);
        builder
    }

    pub(super) fn initial_events(self) -> EntityEvents<PayoutEvent> {
        let mut events = EntityEvents::init([
            PayoutEvent::PayoutInitialized {
                id: self.id,
                wallet_id: self.wallet_id,
                batch_group_id: self.batch_group_id,
                profile_id: self.profile_id,
                destination: self.destination,
                satoshis: self.satoshis,
            },
            PayoutEvent::PayoutExternalIdUpdated {
                external_id: self.external_id,
            },
        ]);
        if let Some(metadata) = self.metadata {
            events.push(PayoutEvent::PayoutMetadataUpdated { metadata });
        }
        events
    }
}

impl TryFrom<EntityEvents<PayoutEvent>> for UnbatchedPayout {
    type Error = EntityError;

    fn try_from(events: EntityEvents<PayoutEvent>) -> Result<Self, Self::Error> {
        let mut builder = UnbatchedPayoutBuilder::default();
        for event in events.iter() {
            if let PayoutEvent::PayoutInitialized {
                id,
                wallet_id,
                destination,
                satoshis,
                ..
            } = event
            {
                builder = builder
                    .id(*id)
                    .wallet_id(*wallet_id)
                    .destination(destination.clone())
                    .satoshis(*satoshis);
            }
        }
        builder.events(events).build()
    }
}

impl TryFrom<EntityEvents<PayoutEvent>> for Payout {
    type Error = EntityError;

    fn try_from(events: EntityEvents<PayoutEvent>) -> Result<Self, Self::Error> {
        let mut builder = PayoutBuilder::default();
        for event in events.iter() {
            match event {
                PayoutEvent::PayoutInitialized {
                    id,
                    wallet_id,
                    profile_id,
                    batch_group_id,
                    destination,
                    satoshis,
                    ..
                } => {
                    builder = builder
                        .id(*id)
                        .wallet_id(*wallet_id)
                        .profile_id(*profile_id)
                        .batch_group_id(*batch_group_id)
                        .destination(destination.clone())
                        .satoshis(*satoshis);
                }

                PayoutEvent::PayoutExternalIdUpdated { external_id } => {
                    builder = builder.external_id(external_id.clone());
                }
                PayoutEvent::PayoutMetadataUpdated { metadata } => {
                    builder = builder.metadata(metadata.clone());
                }
                PayoutEvent::PayoutAddedToBatch { batch_id } => {
                    builder = builder.batch_id(*batch_id);
                }
            }
        }
        builder.build()
    }
}
