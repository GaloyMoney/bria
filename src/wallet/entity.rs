use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use sqlx_ledger::AccountId as LedgerAccountId;

use crate::{primitives::*, xpub::*};

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
    SingleSig(SingleSigWalletKeyChainConfig),
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SingleSigWalletKeyChainConfig {
    xpub: XPub,
}
impl SingleSigWalletKeyChainConfig {
    pub fn new(xpub: XPub) -> Self {
        Self { xpub }
    }
}

impl From<SingleSigWalletKeyChainConfig> for WalletKeyChainConfig {
    fn from(cfg: SingleSigWalletKeyChainConfig) -> Self {
        WalletKeyChainConfig::SingleSig(cfg)
    }
}
