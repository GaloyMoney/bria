use derive_builder::Builder;
use sqlx_ledger::AccountId as LedgerAccountId;

use super::keychain::*;
use crate::primitives::*;

pub struct Wallet {
    pub id: WalletId,
    pub ledger_account_id: LedgerAccountId,
    pub dust_ledger_account_id: LedgerAccountId,
    pub keychains: Vec<(KeychainId, WalletKeyChainConfig)>,
}

impl Wallet {
    pub fn current_keychain(&self) -> (KeychainId, &WalletKeyChainConfig) {
        let (id, cfg) = &self.keychains[0];
        (*id, cfg)
    }
}

#[derive(Builder, Clone)]
pub struct NewWallet {
    #[builder(setter(into))]
    pub id: WalletId,
    pub(super) name: String,
    #[builder(setter(into))]
    pub(super) keychain: WalletKeyChainConfig,
    pub(super) dust_account_id: LedgerAccountId,
}

impl NewWallet {
    pub fn builder() -> NewWalletBuilder {
        let mut builder = NewWalletBuilder::default();
        builder.id(WalletId::new());
        builder
    }
}
