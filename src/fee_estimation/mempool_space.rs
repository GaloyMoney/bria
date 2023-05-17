use bdk::FeeRate;
use serde::Deserialize;

use crate::{error::*, primitives::TxPriority};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecommendedFeesResponse {
    fastest_fee: u64,
    // half_hour_fee: u64,
    hour_fee: u64,
    economy_fee: u64,
    // minimum_fee: u64,
}
pub struct MempoolSpaceClient {}

impl MempoolSpaceClient {
    pub async fn fee_rate(priority: TxPriority) -> Result<FeeRate, BriaError> {
        let url = "http://localhost:8999/api/v1/fees/recommended";
        let resp = reqwest::get(url).await.map_err(BriaError::FeeEstimation)?;
        let fee_estimations: RecommendedFeesResponse =
            resp.json().await.map_err(BriaError::FeeEstimation)?;
        match priority {
            TxPriority::Economy => Ok(FeeRate::from_sat_per_vb(fee_estimations.economy_fee as f32)),
            TxPriority::OneHour => Ok(FeeRate::from_sat_per_vb(fee_estimations.hour_fee as f32)),
            TxPriority::NextBlock => {
                Ok(FeeRate::from_sat_per_vb(fee_estimations.fastest_fee as f32))
            }
        }
    }
}
