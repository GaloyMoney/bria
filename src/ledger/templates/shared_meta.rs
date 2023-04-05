use serde::{Deserialize, Serialize};

use crate::primitives::Satoshis;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub total_utxo_in_sats: Satoshis,
    pub total_utxo_settled_in_sats: Satoshis,
    pub change_sats: Satoshis,
    pub fee_sats: Satoshis,
}
