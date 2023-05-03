use crate::{address::*, error::*, primitives::*};

use super::event::*;

pub struct Augmentation {
    address: Option<AddressAugmentation>,
}

pub struct AddressAugmentation {
    address: bitcoin::Address,
    wallet_id: WalletId,
    external_id: String,
    metadata: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct Augmenter {
    addresses: Addresses,
}

impl Augmenter {
    pub fn new(addresses: &Addresses) -> Self {
        Self {
            addresses: addresses.clone(),
        }
    }

    pub async fn load_augmentation(
        &self,
        account_id: AccountId,
        payload: OutboxEventPayload,
    ) -> Result<Augmentation, BriaError> {
        match payload {
            OutboxEventPayload::UtxoDetected {
                address, wallet_id, ..
            } => {
                let address_info = self
                    .addresses
                    .find_by_address(account_id, address.clone())
                    .await?;
                Ok(Augmentation {
                    address: Some(AddressAugmentation {
                        address,
                        wallet_id,
                        metadata: address_info.metadata().cloned(),
                        external_id: address_info.external_id,
                    }),
                })
            }
            OutboxEventPayload::UtxoSettled {
                address, wallet_id, ..
            } => {
                let address_info = self
                    .addresses
                    .find_by_address(account_id, address.clone())
                    .await?;
                Ok(Augmentation {
                    address: Some(AddressAugmentation {
                        address,
                        wallet_id,
                        metadata: address_info.metadata().cloned(),
                        external_id: address_info.external_id,
                    }),
                })
            }
        }
    }
}

impl From<OutboxEvent<WithoutAugmentation>> for OutboxEvent<Augmentation> {
    fn from(event: OutboxEvent<WithoutAugmentation>) -> Self {
        Self {
            id: event.id,
            account_id: event.account_id,
            sequence: event.sequence,
            payload: event.payload,
            ledger_event_id: event.ledger_event_id,
            ledger_tx_id: event.ledger_tx_id,
            recorded_at: event.recorded_at,
            augmentation: None,
        }
    }
}
