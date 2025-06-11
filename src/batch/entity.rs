use derive_builder::Builder;

use std::collections::HashMap;

use crate::primitives::*;

use super::error::BatchError;

pub struct Batch {
    pub id: BatchId,
    pub account_id: AccountId,
    pub payout_queue_id: PayoutQueueId,
    pub bitcoin_tx_id: bitcoin::Txid,
    pub wallet_summaries: HashMap<WalletId, WalletSummary>,
    pub unsigned_psbt: bitcoin::psbt::PartiallySignedTransaction,
    pub signed_tx: Option<bitcoin::Transaction>,
}

impl Batch {
    pub fn get_tx_to_broadcast(&self) -> Option<bitcoin::Transaction> {
        if self.accounting_complete() && self.is_signed() && !self.is_cancelled() {
            self.signed_tx.clone()
        } else {
            None
        }
    }

    pub fn accounting_complete(&self) -> bool {
        self.wallet_summaries
            .values()
            .all(|s| s.batch_created_ledger_tx_id.is_some())
    }

    pub fn validate_cancellation(&self) -> Result<(), BatchError> {
        if self.is_cancelled() {
            return Err(BatchError::BatchAlreadyCancelled);
        }

        if self.is_signed() {
            return Err(BatchError::BatchAlreadySigned);
        }

        Ok(())
    }

    pub fn is_cancelled(&self) -> bool {
        self.wallet_summaries
            .values()
            .all(|s| s.batch_cancel_ledger_tx_id.is_some())
    }

    pub fn is_signed(&self) -> bool {
        self.signed_tx.is_some()
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
    pub batch_cancel_ledger_tx_id: Option<LedgerTransactionId>,
}
