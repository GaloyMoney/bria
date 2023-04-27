use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::{ledger::JournalEventMetadata, primitives::*};

#[derive(Builder, Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub id: OutboxEventId,
    pub account_id: AccountId,
    pub sequence: EventSequence,
    pub payload: OutboxEventPayload,
    #[builder(default, setter(strip_option))]
    pub ledger_event_id: Option<SqlxLedgerEventId>,
    #[builder(default, setter(strip_option))]
    pub ledger_tx_id: Option<LedgerTransactionId>,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
}

impl OutboxEvent {
    pub fn builder() -> OutboxEventBuilder {
        let mut builder = OutboxEventBuilder::default();
        builder.id(OutboxEventId::new());
        builder
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
}

impl OutboxEventPayload {
    pub fn _type(&self) -> OutboxEventType {
        match self {
            OutboxEventPayload::UtxoDetected { .. } => OutboxEventType::UtxoDetected,
            OutboxEventPayload::UtxoSettled { .. } => OutboxEventType::UtxoSettled,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboxEventType {
    UtxoDetected,
    UtxoSettled,
}

impl TryFrom<JournalEventMetadata> for OutboxEventPayload {
    type Error = ();

    fn try_from(meta: JournalEventMetadata) -> Result<Self, ()> {
        use JournalEventMetadata::*;
        let res = match meta {
            UtxoDetected(meta) => OutboxEventPayload::UtxoDetected {
                tx_id: meta.outpoint.txid,
                vout: meta.outpoint.vout,
                satoshis: meta.satoshis,
                address: meta.address,
                wallet_id: meta.wallet_id,
                keychain_id: meta.keychain_id,
            },
            UtxoSettled(meta) => OutboxEventPayload::UtxoSettled {
                tx_id: meta.outpoint.txid,
                vout: meta.outpoint.vout,
                satoshis: meta.satoshis,
                address: meta.address,
                wallet_id: meta.wallet_id,
                keychain_id: meta.keychain_id,
                confirmation_time: meta.confirmation_time,
            },
            _ => return Err(()),
        };
        Ok(res)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventSequence(u64);
impl EventSequence {
    pub(super) const BEGIN: Self = EventSequence(0);
    pub(super) fn next(&self) -> Self {
        Self(self.0 + 1)
    }
}
impl From<i64> for EventSequence {
    fn from(seq: i64) -> Self {
        Self(seq as u64)
    }
}
impl From<EventSequence> for i64 {
    fn from(seq: EventSequence) -> Self {
        seq.0 as i64
    }
}
