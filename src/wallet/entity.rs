use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use sqlx_ledger::AccountId as LedgerAccountId;

use super::bdk_wallet::*;
use crate::{primitives::*, xpub::*};

pub struct Wallet {
    pub id: WalletId,
    pub ledger_account_id: LedgerAccountId,
    pub dust_ledger_account_id: LedgerAccountId,
    pub keychains: Vec<WalletKeyChainConfig>,
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

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WalletKeyChainConfig {
    Wpkh(WpkhKeyChainConfig),
}

#[derive(Deserialize, Serialize, Clone)]
pub struct WpkhKeyChainConfig {
    xpub: XPub,
}
impl WpkhKeyChainConfig {
    pub fn new(xpub: XPub) -> Self {
        Self { xpub }
    }
}

impl IntoExternalDescriptor for &WpkhKeyChainConfig {
    fn into_external_descriptor(self) -> String {
        format!("wpkh({}/0/*)", *self.xpub)
    }
}
impl IntoInternalDescriptor for &WpkhKeyChainConfig {
    fn into_internal_descriptor(self) -> String {
        format!("wpkh({}/1/*)", *self.xpub)
    }
}

impl From<WpkhKeyChainConfig> for WalletKeyChainConfig {
    fn from(cfg: WpkhKeyChainConfig) -> Self {
        WalletKeyChainConfig::Wpkh(cfg)
    }
}
