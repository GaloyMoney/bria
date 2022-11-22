mod wallet;

use crate::xpub::*;
use serde::{Deserialize, Serialize};
pub use wallet::*;

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

impl ToExternalDescriptor for WpkhKeyChainConfig {
    fn to_external_descriptor(&self) -> String {
        format!("wpkh({}/0/*)", *self.xpub)
    }
}
impl ToInternalDescriptor for WpkhKeyChainConfig {
    fn to_internal_descriptor(&self) -> String {
        format!("wpkh({}/1/*)", *self.xpub)
    }
}

impl From<WpkhKeyChainConfig> for WalletKeyChainConfig {
    fn from(cfg: WpkhKeyChainConfig) -> Self {
        WalletKeyChainConfig::Wpkh(cfg)
    }
}
