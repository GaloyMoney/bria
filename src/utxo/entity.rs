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
    pub bdk_spent: bool,
    pub block_height: Option<u32>,
    pub pending_income_ledger_tx_id: LedgerTransactionId,
    pub confirmed_income_ledger_tx_id: Option<LedgerTransactionId>,
    pub spending_batch_id: Option<BatchId>,
}

#[derive(Debug)]
pub struct ConfirmedUtxo {
    pub keychain_id: KeychainId,
    pub value: Satoshis,
    pub address: bitcoin::Address,
    pub pending_income_ledger_tx_id: LedgerTransactionId,
    pub confirmed_income_ledger_tx_id: LedgerTransactionId,
    pub pending_spend_ledger_tx_id: Option<LedgerTransactionId>,
}

#[derive(Debug)]
pub(super) struct SpentUtxo {
    pub outpoint: bitcoin::OutPoint,
    pub value: Satoshis,
    pub change_address: bool,
    pub confirmed: bool,
}

pub struct KeychainUtxos {
    pub keychain_id: KeychainId,
    pub utxos: Vec<WalletUtxo>,
}

#[derive(Builder)]
pub struct NewUtxo {
    pub wallet_id: WalletId,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub kind: KeychainKind,
    pub address_idx: u32,
    #[builder(setter(into))]
    pub value: Satoshis,
    pub address: String,
    pub script_hex: String,
    pub sats_per_vbyte_when_created: f32,
    pub self_pay: bool,
    pub bdk_spent: bool,
    pub income_pending_ledger_tx_id: LedgerTransactionId,
}

impl NewUtxo {
    pub fn builder() -> NewUtxoBuilder {
        let mut builder = NewUtxoBuilder::default();
        builder.income_pending_ledger_tx_id(LedgerTransactionId::new());
        builder
    }
}
