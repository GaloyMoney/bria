use derive_builder::Builder;
use es_entity::*;
use std::collections::{HashMap, HashSet};

use super::entity::PayoutEvent;
use crate::primitives::*;

pub struct UnbatchedPayouts {
    inner: HashMap<WalletId, Vec<UnbatchedPayout>>,
    shifted: HashMap<PayoutId, UnbatchedPayout>,
    simulated_payout: Option<(WalletId, TxPayout)>,
    pub(super) batch_id: Option<BatchId>,
    pub(super) batched: Vec<UnbatchedPayout>,
}

impl UnbatchedPayouts {
    pub(super) fn new(inner: HashMap<WalletId, Vec<UnbatchedPayout>>) -> Self {
        Self {
            inner,
            batch_id: None,
            simulated_payout: None,
            shifted: HashMap::new(),
            batched: Vec::new(),
        }
    }

    pub fn wallet_ids(&self) -> HashSet<WalletId> {
        std::iter::once(self.simulated_payout.as_ref())
            .filter_map(|p| p.map(|(id, _)| id))
            .chain(self.inner.keys())
            .copied()
            .collect()
    }

    pub fn n_payouts(&self) -> usize {
        self.inner.values().fold(0, |acc, v| acc + v.len())
    }

    pub fn include_simulated_payout(&mut self, wallet_id: WalletId, payout: TxPayout) {
        self.simulated_payout = Some((wallet_id, payout));
    }

    pub fn n_not_batched(&self) -> usize {
        self.shifted.len() + self.inner.values().fold(0, |acc, v| acc + v.len())
    }

    pub fn commit_to_batch(
        &mut self,
        bitcoin_tx_id: bitcoin::Txid,
        batch_id: impl Into<BatchId>,
        payout_ids: impl Iterator<Item = (impl Into<PayoutId>, u32)>,
    ) {
        if self.shifted.is_empty() {
            self.shifted.extend(
                self.inner
                    .drain()
                    .flat_map(|(_, payouts)| payouts.into_iter().map(|p| (p.id, p))),
            );
        }
        let batch_id = batch_id.into();
        self.batch_id = Some(batch_id);
        for (id, vout) in payout_ids {
            let mut payout = self
                .shifted
                .remove(&id.into())
                .expect("unbatched payout not found");
            payout.commit_to_batch(
                batch_id,
                bitcoin::OutPoint {
                    txid: bitcoin_tx_id,
                    vout,
                },
            );
            self.batched.push(payout);
        }
    }

    pub fn into_tx_payouts(&self) -> HashMap<WalletId, Vec<TxPayout>> {
        let mut ret: HashMap<WalletId, Vec<TxPayout>> = self
            .inner
            .iter()
            .map(|(wallet_id, payouts)| (*wallet_id, payouts.iter().map(TxPayout::from).collect()))
            .collect();
        if let Some((wallet_id, payout)) = &self.simulated_payout {
            let entry = ret.entry(*wallet_id).or_default();
            entry.insert(0, payout.clone());
        }
        ret
    }
}

#[derive(Builder)]
#[builder(pattern = "owned", build_fn(error = "EsEntityError"))]
pub struct UnbatchedPayout {
    pub id: PayoutId,
    pub wallet_id: WalletId,
    pub destination: PayoutDestination,
    pub satoshis: Satoshis,

    pub(super) events: EntityEvents<PayoutEvent>,
}

impl UnbatchedPayout {
    pub(super) fn commit_to_batch(&mut self, batch_id: BatchId, outpoint: bitcoin::OutPoint) {
        self.events
            .push(PayoutEvent::CommittedToBatch { batch_id, outpoint });
    }
}

impl TryFrom<EntityEvents<PayoutEvent>> for UnbatchedPayout {
    type Error = EsEntityError;

    fn try_from(events: EntityEvents<PayoutEvent>) -> Result<Self, Self::Error> {
        let mut builder = UnbatchedPayoutBuilder::default();
        for event in events.iter_all() {
            if let PayoutEvent::Initialized {
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

impl From<&UnbatchedPayout> for TxPayout {
    fn from(payout: &UnbatchedPayout) -> Self {
        (
            uuid::Uuid::from(payout.id),
            payout.destination.onchain_address().clone(),
            payout.satoshis,
        )
    }
}
