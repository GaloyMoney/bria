use super::{error::OutboxError, event::*};
use crate::{address::*, batch_inclusion::*, payout::*, primitives::*};

pub struct Augmentation {
    pub address: Option<AddressAugmentation>,
    pub payout: Option<PayoutWithInclusionEstimate>,
}

pub struct AddressAugmentation {
    pub address: Address,
    pub wallet_id: WalletId,
    pub external_id: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone)]
pub struct Augmenter {
    addresses: Addresses,
    payouts: Payouts,
    batch_inclusion: BatchInclusion,
}

impl Augmenter {
    pub fn new(addresses: &Addresses, payouts: &Payouts, batch_inclusion: &BatchInclusion) -> Self {
        Self {
            addresses: addresses.clone(),
            payouts: payouts.clone(),
            batch_inclusion: batch_inclusion.clone(),
        }
    }

    pub async fn load_augmentation(
        &self,
        account_id: AccountId,
        payload: OutboxEventPayload,
    ) -> Result<Augmentation, OutboxError> {
        match payload {
            OutboxEventPayload::UtxoDetected {
                address, wallet_id, ..
            }
            | OutboxEventPayload::UtxoSettled {
                address, wallet_id, ..
            }
            | OutboxEventPayload::UtxoDropped {
                address, wallet_id, ..
            } => {
                let address_info = self
                    .addresses
                    .find_by_account_id_and_address(account_id, address.to_string())
                    .await?;
                Ok(Augmentation {
                    address: Some(AddressAugmentation {
                        address,
                        wallet_id,
                        metadata: address_info.metadata().cloned(),
                        external_id: address_info.external_id,
                    }),
                    payout: None,
                })
            }
            OutboxEventPayload::PayoutSubmitted { id, .. }
            | OutboxEventPayload::PayoutCancelled { id, .. }
            | OutboxEventPayload::PayoutCommitted { id, .. }
            | OutboxEventPayload::PayoutBroadcast { id, .. }
            | OutboxEventPayload::PayoutSettled { id, .. } => {
                let payout = self
                    .payouts
                    .find_by_account_id_and_id(account_id, id)
                    .await?;
                let payout = self
                    .batch_inclusion
                    .include_estimate(account_id, payout)
                    .await?;
                Ok(Augmentation {
                    payout: Some(payout),
                    address: None,
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
