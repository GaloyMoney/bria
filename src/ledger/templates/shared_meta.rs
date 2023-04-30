use serde::{Deserialize, Serialize};

use crate::primitives::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletTransactionSummary {
    pub account_id: AccountId,
    pub wallet_id: WalletId,
    pub current_keychain_id: KeychainId,
    pub bitcoin_tx_id: bitcoin::Txid,
    pub total_utxo_in_sats: Satoshis,
    pub total_utxo_settled_in_sats: Satoshis,
    pub change_sats: Satoshis,
    pub fee_sats: Satoshis,
    pub change_outpoint: Option<bitcoin::OutPoint>,
    pub change_address: Option<bitcoin::Address>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchWalletInfo {
    pub account_id: AccountId,
    pub batch_group_id: BatchGroupId,
    pub batch_id: BatchId,
    pub wallet_id: WalletId,
    pub included_payouts: Vec<PayoutInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutInfo {
    pub id: PayoutId,
    pub profile_id: ProfileId,
    pub satoshis: Satoshis,
    pub destination: PayoutDestination,
}
