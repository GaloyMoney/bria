use crate::primitives::{bitcoin::*, *};
use derive_builder::Builder;

pub struct WalletUtxo {}

#[derive(Builder)]
pub struct NewWalletUtxo {
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub kind: KeychainKind,
    pub address_idx: u32,
    #[builder(setter(into))]
    pub value: Satoshis,
    pub address: String,
    pub script_hex: String,
    pub ledger_tx_pending_id: LedgerTransactionId,
}

impl NewWalletUtxo {
    pub fn builder() -> NewWalletUtxoBuilder {
        let mut builder = NewWalletUtxoBuilder::default();
        builder.ledger_tx_pending_id(LedgerTransactionId::new());
        builder
    }
}
