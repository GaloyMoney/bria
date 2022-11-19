use crate::primitives::*;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};

#[derive(Builder, Clone)]
pub struct NewWallet {
    pub id: WalletId,
    pub(super) name: String,
    #[builder(setter(into))]
    pub(super) keychain: WalletKeyChainConfig,
}

impl NewWallet {
    pub fn builder() -> NewWalletBuilder {
        let mut builder = NewWalletBuilder::default();
        builder.id(WalletId::new());
        builder
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum WalletKeyChainConfig {
    SingleSig(SingleSigWalletKeyChainConfig),
}

#[derive(Deserialize, Serialize, Clone)]
pub struct SingleSigWalletKeyChainConfig {
    xpub: String,
}
impl SingleSigWalletKeyChainConfig {
    pub fn new(xpub: String) -> Self {
        Self { xpub }
    }
}

impl From<SingleSigWalletKeyChainConfig> for WalletKeyChainConfig {
    fn from(cfg: SingleSigWalletKeyChainConfig) -> Self {
        WalletKeyChainConfig::SingleSig(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_keychain_config() {
        let cfg = WalletKeyChainConfig::SingleSig(SingleSigWalletKeyChainConfig {
            xpub: "xpub".to_string(),
        });
        // assert_eq!(serde_json::to_string(&cfg).unwrap(), "".to_string());
    }
}
