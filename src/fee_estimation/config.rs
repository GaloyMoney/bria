use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolSpaceConfig {
    #[serde(default = "default_url")]
    pub url: String,
}

impl Default for MempoolSpaceConfig {
    fn default() -> Self {
        Self { url: default_url() }
    }
}

fn default_url() -> String {
    "https://mempool.space/api/v1/fees/recommended".to_string()
}
