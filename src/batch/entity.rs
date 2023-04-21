use derive_builder::Builder;

use std::collections::HashMap;

use crate::primitives::*;

pub struct Batch {
    pub id: BatchId,
    pub account_id: AccountId,
    pub batch_group_id: BatchGroupId,
    pub bitcoin_tx_id: bitcoin::Txid,
    pub wallet_summaries: HashMap<WalletId, WalletSummary>,
    pub included_utxos: HashMap<WalletId, HashMap<KeychainId, Vec<bitcoin::OutPoint>>>,
    pub unsigned_psbt: bitcoin::psbt::PartiallySignedTransaction,
    pub signed_tx: Option<bitcoin::Transaction>,
}

impl Batch {
    pub fn accounting_complete(&self) -> bool {
        self.wallet_summaries
            .values()
            .all(|s| s.create_batch_ledger_tx_id.is_some())
    }
}

#[derive(Builder, Clone)]
pub struct NewBatch {
    pub id: BatchId,
    pub(super) account_id: AccountId,
    pub(super) batch_group_id: BatchGroupId,
    pub(super) tx_id: bitcoin::Txid,
    pub(super) total_fee_sats: Satoshis,
    pub(super) unsigned_psbt: bitcoin::psbt::PartiallySignedTransaction,
    pub(super) wallet_summaries: HashMap<WalletId, WalletSummary>,
    pub(super) included_payouts: HashMap<WalletId, Vec<PayoutId>>,
    pub(super) included_utxos: HashMap<WalletId, HashMap<KeychainId, Vec<bitcoin::OutPoint>>>,
}

impl NewBatch {
    pub fn builder() -> NewBatchBuilder {
        NewBatchBuilder::default()
    }

    pub fn iter_utxos(
        &'_ self,
    ) -> impl Iterator<Item = (WalletId, KeychainId, bitcoin::OutPoint)> + '_ {
        self.included_utxos
            .iter()
            .flat_map(|(wallet_id, keychains)| {
                keychains.iter().map(move |(keychain_id, utxos)| {
                    utxos
                        .iter()
                        .map(move |utxo| (*wallet_id, *keychain_id, *utxo))
                })
            })
            .flatten()
    }
}

#[derive(Clone)]
pub struct WalletSummary {
    pub wallet_id: WalletId,
    pub total_in_sats: Satoshis,
    pub total_spent_sats: Satoshis,
    pub fee_sats: Satoshis,
    pub change_sats: Satoshis,
    pub change_address: bitcoin::Address,
    pub change_outpoint: Option<bitcoin::OutPoint>,
    pub change_keychain_id: KeychainId,
    pub create_batch_ledger_tx_id: Option<LedgerTransactionId>,
    pub submitted_ledger_tx_id: Option<LedgerTransactionId>,
}
