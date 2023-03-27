use crate::primitives::bitcoin::*;
use derive_builder::Builder;

pub struct WalletUtxo {}

#[derive(Builder)]
pub struct NewWalletUtxo {
    outpoint: OutPoint,
    kind: KeychainKind,
    address_path: u32,
    address: String,
    script_hex: String,
    keychain: KeychainKind,
}
