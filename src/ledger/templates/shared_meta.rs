use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use crate::primitives::*;

pub type EncumberedSpendingFees = HashMap<bitcoin::OutPoint, Satoshis>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletTransactionSummary {
    pub account_id: AccountId,
    pub wallet_id: WalletId,
    pub current_keychain_id: KeychainId,
    pub bitcoin_tx_id: bitcoin::Txid,
    pub total_utxo_in_sats: Satoshis,
    pub total_utxo_settled_in_sats: Satoshis,
    pub fee_sats: Satoshis,
    pub change_utxos: Vec<ChangeOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeOutput {
    pub outpoint: bitcoin::OutPoint,
    pub address: bitcoin::Address,
    pub satoshis: Satoshis,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchWalletInfo {
    pub account_id: AccountId,
    pub payout_queue_id: PayoutQueueId,
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
