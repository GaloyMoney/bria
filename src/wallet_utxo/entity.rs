use crate::primitives::{bitcoin::*, *};
use derive_builder::Builder;

pub struct WalletUtxo {
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub kind: KeychainKind,
    pub address_idx: u32,
    pub value: Satoshis,
    pub address: Option<String>,
    pub spent: bool,
    pub block_height: Option<u32>,
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
    pub ledger_tx_pending_id: LedgerTransactionId,
    #[builder(setter(into))]
    pub confirmation_time: Option<BlockTime>,
}

impl NewWalletUtxo {
    pub fn builder() -> NewWalletUtxoBuilder {
        let mut builder = NewWalletUtxoBuilder::default();
        builder.ledger_tx_pending_id(LedgerTransactionId::new());
        builder
    }
}
