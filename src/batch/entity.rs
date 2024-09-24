use derive_builder::Builder;

use std::collections::HashMap;

use crate::primitives::*;

pub struct Batch {
    pub id: BatchId,
    pub account_id: AccountId,
    pub payout_queue_id: PayoutQueueId,
    pub bitcoin_tx_id: bitcoin::Txid,
    pub wallet_summaries: HashMap<WalletId, WalletSummary>,
    pub unsigned_psbt: bitcoin::psbt::PartiallySignedTransaction,
    pub signed_tx: Option<bitcoin::Transaction>,
    pub provisional_proposal: Option<payjoin::receive::v2::ProvisionalProposal>,
}

impl Batch {
    pub fn accounting_complete(&self) -> bool {
        self.wallet_summaries
            .values()
            .all(|s| s.batch_created_ledger_tx_id.is_some())
    }
}

#[derive(Builder, Clone)]
pub struct NewBatch {
    pub id: BatchId,
    pub(super) account_id: AccountId,
    pub(super) payout_queue_id: PayoutQueueId,
    pub(super) tx_id: bitcoin::Txid,
    pub(super) total_fee_sats: Satoshis,
    pub(super) unsigned_psbt: bitcoin::psbt::PartiallySignedTransaction,
    pub(super) wallet_summaries: HashMap<WalletId, WalletSummary>,
    pub(super) provisional_proposal: Option<payjoin::receive::v2::ProvisionalProposal>,
}

impl NewBatch {
    pub fn builder() -> NewBatchBuilder {
        NewBatchBuilder::default()
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct CpfpDetails {
    pub tx_id: bitcoin::Txid,
    pub batch_id: Option<BatchId>,
    pub bump_fee: Satoshis,
}

#[derive(Clone)]
pub struct WalletSummary {
    pub wallet_id: WalletId,
    pub current_keychain_id: KeychainId,
    pub signing_keychains: Vec<KeychainId>,
    pub total_in_sats: Satoshis,
    pub total_spent_sats: Satoshis,
    pub total_fee_sats: Satoshis,
    pub cpfp_fee_sats: Satoshis,
    pub cpfp_details: HashMap<bitcoin::OutPoint, HashMap<bitcoin::Txid, CpfpDetails>>,
    pub change_sats: Satoshis,
    pub change_address: Option<Address>,
    pub change_outpoint: Option<bitcoin::OutPoint>,
    pub batch_created_ledger_tx_id: Option<LedgerTransactionId>,
    pub batch_broadcast_ledger_tx_id: Option<LedgerTransactionId>,
}
