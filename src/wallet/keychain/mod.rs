mod wallet;

use crate::xpub::*;
use serde::{Deserialize, Serialize};
pub use wallet::*;

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WalletKeyChainConfig {
    Wpkh(WpkhKeyChainConfig),
    Tr(TrKeyChainConfig),
}

impl WalletKeyChainConfig {
    pub fn xpubs(&self) -> Vec<XPub> {
        match self {
            WalletKeyChainConfig::Wpkh(cfg) => vec![cfg.xpub.clone()],
            WalletKeyChainConfig::Tr(cfg) => vec![cfg.xpub.clone()],
        }
    }
}

impl ToExternalDescriptor for WalletKeyChainConfig {
    fn to_external_descriptor(&self) -> String {
        match self {
            Self::Wpkh(cfg) => cfg.to_external_descriptor(),
            Self::Tr(cfg) => cfg.to_external_descriptor(),
        }
    }
}
impl ToInternalDescriptor for WalletKeyChainConfig {
    fn to_internal_descriptor(&self) -> String {
        match self {
            Self::Wpkh(cfg) => cfg.to_internal_descriptor(),
            Self::Tr(cfg) => cfg.to_internal_descriptor(),
        }
    }
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

impl ToExternalDescriptor for WpkhKeyChainConfig {
    fn to_external_descriptor(&self) -> String {
        format!("wpkh({}/0/*)", self.xpub)
    }
}
impl ToInternalDescriptor for WpkhKeyChainConfig {
    fn to_internal_descriptor(&self) -> String {
        format!("wpkh({}/1/*)", self.xpub)
    }
}

impl From<WpkhKeyChainConfig> for WalletKeyChainConfig {
    fn from(cfg: WpkhKeyChainConfig) -> Self {
        WalletKeyChainConfig::Wpkh(cfg)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct TrKeyChainConfig {
    xpub: XPub,
}

impl TrKeyChainConfig {
    pub fn new(xpub: XPub) -> Self {
        Self { xpub }
    }
}

impl ToExternalDescriptor for TrKeyChainConfig {
    fn to_external_descriptor(&self) -> String {
        format!("tr({}/0/*)", self.xpub)
    }
}
impl ToInternalDescriptor for TrKeyChainConfig {
    fn to_internal_descriptor(&self) -> String {
        format!("tr({}/1/*)", self.xpub)
    }
}

impl From<TrKeyChainConfig> for WalletKeyChainConfig {
    fn from(cfg: TrKeyChainConfig) -> Self {
        WalletKeyChainConfig::Tr(cfg)
    }
}
