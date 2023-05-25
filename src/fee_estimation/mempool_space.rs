use bdk::FeeRate;
use serde::{Deserialize, Serialize};

use crate::{error::*, primitives::TxPriority};

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
    url: String,
}

impl MempoolSpaceClient {
    pub fn new(config: MempoolSpaceConfig) -> Self {
        Self { url: config.url }
    }

    pub async fn fee_rate(&self, priority: TxPriority) -> Result<FeeRate, BriaError> {
        let url = format!("{}{}", self.url, "/api/v1/fees/recommended");
        let resp = reqwest::get(url).await.map_err(BriaError::FeeEstimation)?;
        let fee_estimations: RecommendedFeesResponse =
            resp.json().await.map_err(BriaError::FeeEstimation)?;
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
    "https://mempool.space".to_string()
}
