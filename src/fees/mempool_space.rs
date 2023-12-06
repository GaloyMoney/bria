use bdk::FeeRate;
use serde::{Deserialize, Serialize};

use super::error::FeeEstimationError;
use crate::primitives::TxPriority;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecommendedFeesResponse {
    fastest_fee: u64,
    half_hour_fee: u64,
    hour_fee: u64,
    // economy_fee: u64,
    // minimum_fee: u64,
}

#[derive(Clone, Debug)]
pub struct MempoolSpaceClient {
    config: MempoolSpaceConfig,
}

impl MempoolSpaceClient {
    pub fn new(config: MempoolSpaceConfig) -> Self {
        Self { config }
    }

    pub async fn fee_rate(&self, priority: TxPriority) -> Result<FeeRate, FeeEstimationError> {
        let client = reqwest::Client::builder()
            .timeout(self.config.timeout)
            .build()
            .expect("Could not build reqwest client");

        let url = format!("{}{}", self.config.url, "/api/v1/fees/recommended");
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(FeeEstimationError::FeeEstimation)?;
        let fee_estimations = resp
            .json::<RecommendedFeesResponse>()
            .await
            .map_err(FeeEstimationError::CouldNotDecodeResponseBody)?;
        match priority {
            TxPriority::HalfHour => Ok(FeeRate::from_sat_per_vb(
                fee_estimations.half_hour_fee as f32,
            )),
            TxPriority::OneHour => Ok(FeeRate::from_sat_per_vb(fee_estimations.hour_fee as f32)),
            TxPriority::NextBlock => {
                Ok(FeeRate::from_sat_per_vb(fee_estimations.fastest_fee as f32))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde_with::serde_as]
pub struct MempoolSpaceConfig {
    #[serde(default = "default_url")]
    pub url: String,
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(default = "default_timeout")]
    pub timeout: std::time::Duration,
}

impl Default for MempoolSpaceConfig {
    fn default() -> Self {
        Self {
            url: default_url(),
            timeout: default_timeout(),
        }
    }
}

fn default_url() -> String {
    "https://mempool.space".to_string()
}

fn default_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(10)
}
