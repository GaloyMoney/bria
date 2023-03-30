use crate::primitives::{bitcoin::*, *};
use derive_builder::Builder;

pub struct WalletUtxo {
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub kind: KeychainKind,
    pub address_idx: u32,
    pub value: Satoshis,
    pub address: Option<bitcoin::Address>,
    pub spent: bool,
    pub block_height: Option<u32>,
    pub income_pending_ledger_tx_id: Option<LedgerTransactionId>,
    pub income_settled_ledger_tx_id: Option<LedgerTransactionId>,
    pub spending_batch_id: Option<BatchId>,
}

pub struct ConfimedIncomeUtxo {
    pub keychain_id: KeychainId,
    pub address_idx: u32,
    pub value: Satoshis,
    pub address: bitcoin::Address,
    pub block_height: u32,
    pub income_pending_ledger_tx_id: LedgerTransactionId,
    pub income_settled_ledger_tx_id: LedgerTransactionId,
    pub spending_batch_id: Option<BatchId>,
}

pub struct KeychainUtxos {
    pub keychain_id: KeychainId,
    pub utxos: Vec<WalletUtxo>,
}

#[derive(Builder)]
pub struct NewWalletUtxo {
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub kind: KeychainKind,
    pub address_idx: u32,
    #[builder(setter(into))]
    pub value: Satoshis,
    pub address: String,
    pub script_hex: String,
    pub spent: bool,
    pub income_pending_ledger_tx_id: LedgerTransactionId,
}

impl NewWalletUtxo {
    pub fn builder() -> NewWalletUtxoBuilder {
        let mut builder = NewWalletUtxoBuilder::default();
        builder.income_pending_ledger_tx_id(LedgerTransactionId::new());
        builder
    }
}
