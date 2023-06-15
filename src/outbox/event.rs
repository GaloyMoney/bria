use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::{
    fees,
    ledger::{BatchBroadcastMeta, BatchCreatedMeta, JournalEventMetadata, SpendSettledMeta},
    primitives::*,
};

pub type WithoutAugmentation = ();

#[derive(Builder, Debug, Serialize, Deserialize)]
#[builder(pattern = "owned")]
pub struct OutboxEvent<T> {
    pub id: OutboxEventId,
    pub account_id: AccountId,
    pub sequence: EventSequence,
    pub payload: OutboxEventPayload,
    #[builder(default, setter(strip_option))]
    pub ledger_event_id: Option<SqlxLedgerEventId>,
    #[builder(default, setter(strip_option))]
    pub ledger_tx_id: Option<LedgerTransactionId>,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    #[builder(default)]
    #[serde(skip)]
    pub augmentation: Option<T>,
}

impl<T> OutboxEvent<T> {
    pub fn builder() -> OutboxEventBuilder<T> {
        OutboxEventBuilder::default().id(OutboxEventId::new())
    }
}

impl Clone for OutboxEvent<WithoutAugmentation> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            account_id: self.account_id,
            sequence: self.sequence,
            payload: self.payload.clone(),
            ledger_event_id: self.ledger_event_id,
            ledger_tx_id: self.ledger_tx_id,
            recorded_at: self.recorded_at,
            augmentation: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboxEventPayload {
    UtxoDetected {
        tx_id: bitcoin::Txid,
        vout: u32,
        satoshis: Satoshis,
        address: bitcoin::Address,
        wallet_id: WalletId,
        keychain_id: KeychainId,
    },
    UtxoSettled {
        tx_id: bitcoin::Txid,
        vout: u32,
        satoshis: Satoshis,
        address: bitcoin::Address,
        wallet_id: WalletId,
        keychain_id: KeychainId,
        confirmation_time: bitcoin::BlockTime,
    },
    PayoutSubmitted {
        id: PayoutId,
        profile_id: ProfileId,
        wallet_id: WalletId,
        payout_queue_id: PayoutQueueId,
        satoshis: Satoshis,
        destination: PayoutDestination,
    },
    PayoutCommitted {
        id: PayoutId,
        vout: u32,
        profile_id: ProfileId,
        wallet_id: WalletId,
        payout_queue_id: PayoutQueueId,
        batch_id: BatchId,
        tx_id: bitcoin::Txid,
        satoshis: Satoshis,
        destination: PayoutDestination,
        proportional_fee: Satoshis,
    },
    PayoutBroadcast {
        id: PayoutId,
        vout: u32,
        profile_id: ProfileId,
        wallet_id: WalletId,
        payout_queue_id: PayoutQueueId,
        batch_id: BatchId,
        tx_id: bitcoin::Txid,
        satoshis: Satoshis,
        destination: PayoutDestination,
        proportional_fee: Satoshis,
    },
    PayoutSettled {
        id: PayoutId,
        vout: u32,
        profile_id: ProfileId,
        wallet_id: WalletId,
        payout_queue_id: PayoutQueueId,
        batch_id: BatchId,
        tx_id: bitcoin::Txid,
        satoshis: Satoshis,
        destination: PayoutDestination,
        proportional_fee: Satoshis,
    },
}

impl From<JournalEventMetadata> for Vec<OutboxEventPayload> {
    fn from(meta: JournalEventMetadata) -> Self {
        use JournalEventMetadata::*;
        let mut res = Vec::new();
        match meta {
            UtxoDetected(meta) => res.push(OutboxEventPayload::UtxoDetected {
                tx_id: meta.outpoint.txid,
                vout: meta.outpoint.vout,
                satoshis: meta.satoshis,
                address: meta.address,
                wallet_id: meta.wallet_id,
                keychain_id: meta.keychain_id,
            }),
            UtxoSettled(meta) => res.push(OutboxEventPayload::UtxoSettled {
                tx_id: meta.outpoint.txid,
                vout: meta.outpoint.vout,
                satoshis: meta.satoshis,
                address: meta.address,
                wallet_id: meta.wallet_id,
                keychain_id: meta.keychain_id,
                confirmation_time: meta.confirmation_time,
            }),
            PayoutSubmitted(meta) => res.push(OutboxEventPayload::PayoutSubmitted {
                id: meta.payout_id,
                wallet_id: meta.wallet_id,
                payout_queue_id: meta.payout_queue_id,
                profile_id: meta.profile_id,
                satoshis: meta.satoshis,
                destination: meta.destination,
            }),
            BatchCreated(BatchCreatedMeta {
                batch_info,
                tx_summary,
            }) => {
                let mut proportional_fees = fees::allocate_proportional_fees(
                    tx_summary.fee_sats,
                    batch_info
                        .included_payouts
                        .iter()
                        .map(|p| (p.id, p.satoshis)),
                );
                for payout in batch_info.included_payouts {
                    res.push(OutboxEventPayload::PayoutCommitted {
                        id: payout.id,
                        vout: payout.vout_in_tx,
                        wallet_id: batch_info.wallet_id,
                        payout_queue_id: batch_info.payout_queue_id,
                        batch_id: batch_info.batch_id,
                        profile_id: payout.profile_id,
                        tx_id: tx_summary.bitcoin_tx_id,
                        satoshis: payout.satoshis,
                        destination: payout.destination,
                        proportional_fee: proportional_fees
                            .remove(&payout.id)
                            .expect("couldn't find proportional fee"),
                    })
                }
            }
            BatchBroadcast(BatchBroadcastMeta {
                batch_info,
                tx_summary,
                ..
            }) => {
                let mut proportional_fees = fees::allocate_proportional_fees(
                    tx_summary.fee_sats,
                    batch_info
                        .included_payouts
                        .iter()
                        .map(|p| (p.id, p.satoshis)),
                );
                for payout in batch_info.included_payouts {
                    res.push(OutboxEventPayload::PayoutBroadcast {
                        id: payout.id,
                        vout: payout.vout_in_tx,
                        wallet_id: batch_info.wallet_id,
                        payout_queue_id: batch_info.payout_queue_id,
                        batch_id: batch_info.batch_id,
                        profile_id: payout.profile_id,
                        tx_id: tx_summary.bitcoin_tx_id,
                        satoshis: payout.satoshis,
                        destination: payout.destination,
                        proportional_fee: proportional_fees
                            .remove(&payout.id)
                            .expect("couldn't find proportional fee"),
                    })
                }
            }
            SpendSettled(SpendSettledMeta {
                batch_info: Some(batch_info),
                tx_summary,
                ..
            }) => {
                let mut proportional_fees = fees::allocate_proportional_fees(
                    tx_summary.fee_sats,
                    batch_info
                        .included_payouts
                        .iter()
                        .map(|p| (p.id, p.satoshis)),
                );
                for payout in batch_info.included_payouts {
                    res.push(OutboxEventPayload::PayoutSettled {
                        id: payout.id,
                        vout: payout.vout_in_tx,
                        wallet_id: batch_info.wallet_id,
                        payout_queue_id: batch_info.payout_queue_id,
                        batch_id: batch_info.batch_id,
                        profile_id: payout.profile_id,
                        tx_id: tx_summary.bitcoin_tx_id,
                        satoshis: payout.satoshis,
                        destination: payout.destination,
                        proportional_fee: proportional_fees
                            .remove(&payout.id)
                            .expect("couldn't find proportional fee"),
                    })
                }
            }
            _ => (),
        };
        res
    }
}

#[derive(
    sqlx::Type, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Copy, Clone, Serialize, Deserialize,
)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct EventSequence(i64);
impl EventSequence {
    pub(super) const BEGIN: Self = EventSequence(0);
    pub(super) fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}

impl From<u64> for EventSequence {
    fn from(n: u64) -> Self {
        Self(n as i64)
    }
}

impl From<EventSequence> for u64 {
    fn from(EventSequence(n): EventSequence) -> Self {
        n as u64
    }
}
