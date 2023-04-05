use serde::{Deserialize, Serialize};

use crate::primitives::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub total_utxo_in_sats: Satoshis,
    pub total_utxo_settled_in_sats: Satoshis,
    pub change_sats: Satoshis,
    pub fee_sats: Satoshis,
    pub change_outpoint: Option<bitcoin::OutPoint>,
    pub change_address: Option<bitcoin::Address>,
}
