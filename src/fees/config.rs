use serde::{Deserialize, Serialize};

use super::blockstream::BlockstreamConfig;
use super::mempool_space::MempoolSpaceConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeesConfig {
    #[serde(default)]
    pub mempool_space: MempoolSpaceConfig,
    #[serde(default)]
    pub blockstream: BlockstreamConfig,
}
