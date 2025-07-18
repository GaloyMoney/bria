use derive_builder::Builder;
use es_entity::*;
use serde::{Deserialize, Serialize};

use crate::primitives::*;

use super::error::PayoutError;

#[derive(EsEvent, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[es_event(id = "PayoutId")]
pub enum PayoutEvent {
    Initialized {
        id: PayoutId,
        account_id: AccountId,
        wallet_id: WalletId,
        payout_queue_id: PayoutQueueId,
        profile_id: ProfileId,
        destination: PayoutDestination,
        satoshis: Satoshis,
    },
    ExternalIdUpdated {
        external_id: String,
    },
    MetadataUpdated {
        metadata: serde_json::Value,
    },
    CommittedToBatch {
        batch_id: BatchId,
        outpoint: bitcoin::OutPoint,
    },
    Cancelled {
        executed_by: ProfileId,
    },
}

#[derive(EsEntity, Builder)]
#[builder(pattern = "owned", build_fn(error = "EsEntityError"))]
pub struct Payout {
    pub id: PayoutId,
    pub account_id: AccountId,
    pub wallet_id: WalletId,
    pub profile_id: ProfileId,
    pub payout_queue_id: PayoutQueueId,
    #[builder(setter(into), default)]
    pub batch_id: Option<BatchId>,
    #[builder(setter(into), default)]
    pub outpoint: Option<bitcoin::OutPoint>,
    pub satoshis: Satoshis,
    pub destination: PayoutDestination,
    pub external_id: String,
    #[builder(setter(into), default)]
    pub metadata: Option<serde_json::Value>,

    pub(super) events: EntityEvents<PayoutEvent>,
}

impl Payout {
    pub fn cancel_payout(&mut self, profile_id: ProfileId) -> Result<(), PayoutError> {
        if self.is_cancelled() {
            return Err(PayoutError::PayoutAlreadyCancelled);
        }
        if self.is_already_committed() {
            return Err(PayoutError::PayoutAlreadyCommitted);
        }
        self.events.push(PayoutEvent::Cancelled {
            executed_by: profile_id,
        });
        Ok(())
    }

    pub fn is_cancelled(&self) -> bool {
        for event in self.events.iter_all() {
            if let PayoutEvent::Cancelled { .. } = event {
                return true;
            }
        }
        false
    }

    fn is_already_committed(&self) -> bool {
        self.batch_id.is_some()
    }
}

impl TryFromEvents<PayoutEvent> for Payout {
    fn try_from_events(events: EntityEvents<PayoutEvent>) -> Result<Self, EsEntityError> {
        let mut builder = PayoutBuilder::default();
        for event in events.iter_all() {
            match event {
                PayoutEvent::Initialized {
                    id,
                    account_id,
                    wallet_id,
                    profile_id,
                    payout_queue_id,
                    destination,
                    satoshis,
                    ..
                } => {
                    builder = builder
                        .id(*id)
                        .account_id(*account_id)
                        .wallet_id(*wallet_id)
                        .profile_id(*profile_id)
                        .payout_queue_id(*payout_queue_id)
                        .destination(destination.clone())
                        .satoshis(*satoshis);
                }

                PayoutEvent::ExternalIdUpdated { external_id } => {
                    builder = builder.external_id(external_id.clone());
                }
                PayoutEvent::MetadataUpdated { metadata } => {
                    builder = builder.metadata(metadata.clone());
                }
                PayoutEvent::CommittedToBatch { batch_id, outpoint } => {
                    builder = builder.batch_id(*batch_id).outpoint(*outpoint);
                }
                _ => (),
            }
        }
        builder.events(events).build()
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
    pub(super) payout_queue_id: PayoutQueueId,
    pub(super) profile_id: ProfileId,
    pub(super) satoshis: Satoshis,
    pub(super) destination: PayoutDestination,
    #[builder(setter(into))]
    pub(super) external_id: String,
    #[builder(default, setter(into))]
    pub(super) metadata: Option<serde_json::Value>,
}

impl NewPayout {
    pub fn builder(id: PayoutId) -> NewPayoutBuilder {
        let mut builder = NewPayoutBuilder::default();
        builder.external_id(id.to_string()).id(id);
        builder
    }
}

impl IntoEvents<PayoutEvent> for NewPayout {
    fn into_events(self) -> EntityEvents<PayoutEvent> {
        let mut events = vec![
            PayoutEvent::Initialized {
                id: self.id,
                account_id: self.account_id,
                wallet_id: self.wallet_id,
                payout_queue_id: self.payout_queue_id,
                profile_id: self.profile_id,
                destination: self.destination,
                satoshis: self.satoshis,
            },
            PayoutEvent::ExternalIdUpdated {
                external_id: self.external_id,
            },
        ];
        if let Some(metadata) = self.metadata {
            events.push(PayoutEvent::MetadataUpdated { metadata });
        }
        EntityEvents::init(self.id, events)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;

    fn init_events() -> EntityEvents<PayoutEvent> {
        let id = PayoutId::new();
        EntityEvents::init(
            id,
            [
                PayoutEvent::Initialized {
                    id: id,
                    account_id: AccountId::new(),
                    wallet_id: WalletId::new(),
                    profile_id: ProfileId::new(),
                    payout_queue_id: PayoutQueueId::new(),
                    destination: PayoutDestination::OnchainAddress {
                        value: "bc1qwqdg6squsna38e46795at95yu9atm8azzmyvckulcc7kytlcckxswvvzej"
                            .parse::<Address>()
                            .unwrap(),
                    },
                    satoshis: Satoshis::from(Decimal::from(21)),
                },
                PayoutEvent::ExternalIdUpdated {
                    external_id: "external_id".to_string(),
                },
            ],
        )
    }

    #[test]
    fn cancel_payout() {
        let mut payout = Payout::try_from_events(init_events()).unwrap();
        assert!(payout.cancel_payout(payout.profile_id).is_ok());
        assert!(matches!(
            payout.events.iter_all().last().unwrap(),
            PayoutEvent::Cancelled { .. }
        ));
    }

    #[test]
    fn can_only_cancel_payout_one_time() {
        let mut events = init_events();
        events.push(PayoutEvent::Cancelled {
            executed_by: ProfileId::new(),
        });
        let mut payout = Payout::try_from_events(events).unwrap();
        let result = payout.cancel_payout(payout.profile_id);
        assert!(matches!(result, Err(PayoutError::PayoutAlreadyCancelled)));
    }

    #[test]
    fn errors_when_payout_already_committed() {
        let mut events = init_events();
        events.push(PayoutEvent::CommittedToBatch {
            batch_id: BatchId::new(),
            outpoint: bitcoin::OutPoint {
                txid: "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
                    .parse()
                    .unwrap(),
                vout: 0,
            },
        });

        let mut payout = Payout::try_from_events(events).unwrap();

        let result = payout.cancel_payout(payout.profile_id);
        assert!(matches!(result, Err(PayoutError::PayoutAlreadyCommitted)));
    }
}
