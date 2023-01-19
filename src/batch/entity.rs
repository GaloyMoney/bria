use bitcoin::{blockdata::transaction::OutPoint, util::psbt, Address, Txid};
use derive_builder::Builder;
use sqlx_ledger::TransactionId;

use std::collections::HashMap;

use crate::primitives::*;

pub struct Batch {
    pub id: BatchId,
    pub batch_group_id: BatchGroupId,
    pub bitcoin_tx_id: Txid,
    pub wallet_summaries: HashMap<WalletId, WalletSummary>,
}

#[derive(Builder, Clone)]
pub struct NewBatch {
    pub id: BatchId,
    pub(super) batch_group_id: BatchGroupId,
    pub(super) tx_id: Txid,
    pub(super) total_fee_sats: Satoshis,
    pub(super) unsigned_psbt: psbt::PartiallySignedTransaction,
    pub(super) wallet_summaries: HashMap<WalletId, WalletSummary>,
    pub(super) included_payouts: HashMap<WalletId, Vec<PayoutId>>,
    pub included_utxos: HashMap<KeychainId, Vec<OutPoint>>,
}

impl NewBatch {
    pub fn builder() -> NewBatchBuilder {
        NewBatchBuilder::default()
    }
}

#[derive(Clone)]
pub struct WalletSummary {
    pub wallet_id: WalletId,
    pub total_in_sats: Satoshis,
    pub total_out_sats: Satoshis,
    pub fee_sats: Satoshis,
    pub change_sats: Satoshis,
    pub change_address: Address,
    pub ledger_tx_pending_id: TransactionId,
    pub ledger_tx_settled_id: TransactionId,
}
