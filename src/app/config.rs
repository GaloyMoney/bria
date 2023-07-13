use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;

use crate::{
    fees::MempoolSpaceConfig,
    job::JobsConfig,
    primitives::{
        bitcoin::{self, Network},
        PayoutDestination,
    },
    xpub::SignerEncryptionConfig,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecatedEncyptionKey {
    pub nonce: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub blockchain: BlockchainConfig,
    #[serde(default)]
    pub jobs: JobsConfig,
    #[serde(default)]
    pub signer_encryption: SignerEncryptionConfig,
    pub deprecated_encryption_key: Option<DeprecatedEncyptionKey>,
    #[serde(default)]
    pub fees: FeesConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    #[serde(default = "default_network", deserialize_with = "deserialize_network")]
    pub network: Network,
    #[serde(default = "default_electrum_url")]
    pub electrum_url: String,
}

impl Default for BlockchainConfig {
    fn default() -> Self {
        Self {
            network: default_network(),
            electrum_url: default_electrum_url(),
        }
    }
}

fn default_network() -> Network {
    Network::Regtest
}

fn default_electrum_url() -> String {
    "127.0.0.1:50001".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeesConfig {
    #[serde(default)]
    pub mempool_space: MempoolSpaceConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    blocked_addresses: HashSet<bitcoin::Address>,
}

impl SecurityConfig {
    pub fn is_blocked(&self, destination: &PayoutDestination) -> bool {
        if let Some(addr) = destination.onchain_address() {
            self.blocked_addresses.contains(&addr)
        } else {
            false
        }
    }
}

fn deserialize_network<'de, D>(deserializer: D) -> Result<Network, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    match s.as_str() {
        "mainnet" => Ok(Network::Bitcoin),
        "testnet" => Ok(Network::Testnet),
        "signet" => Ok(Network::Signet),
        "regtest" => Ok(Network::Regtest),
        "bitcoin" => Ok(Network::Bitcoin),
        _ => Err(serde::de::Error::unknown_variant(
            &s,
            &["mainnet", "testnet", "signet", "regtest"],
        )),
    }
}
