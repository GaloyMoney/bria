use serde::{Deserialize, Serialize};

use crate::primitives::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEvent {
    id: OutboxEventId,
    account_id: AccountId,
    sequence: u64,
    payload: OutboxEventPayload,
    recorded_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutboxEventPayload {
    UtxoDetected {
        outpoint: bitcoin::OutPoint,
        satoshis: Satoshis,
        keychain_id: KeychainId,
        wallet_id: WalletId,
        address: bitcoin::Address,
        external_id_when_detected: String,
        address_metadata_when_detected: serde_json::Value,
    },
}
