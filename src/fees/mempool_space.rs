use bdk::FeeRate;
use serde::{Deserialize, Serialize};
use tracing::instrument;

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

    #[instrument(name = "mempool_space.fee_rate", skip(self), ret, err)]
    pub async fn fee_rate(&self, priority: TxPriority) -> Result<FeeRate, FeeEstimationError> {
        let min_retry_interval = std::time::Duration::from_secs(1);
        let max_retry_interval = std::time::Duration::from_secs(30 * 60);
        let retry_policy = reqwest_retry::policies::ExponentialBackoff::builder()
            .retry_bounds(min_retry_interval, max_retry_interval)
            .build_with_max_retries(self.config.number_of_retries);
        let client = reqwest_middleware::ClientBuilder::new(
            reqwest::Client::builder()
                .timeout(self.config.timeout)
                .build()
                .expect("could not build reqwest client"),
        )
        .with(reqwest_retry::RetryTransientMiddleware::new_with_policy(
            retry_policy,
        ))
        .build();

        let url = format!("{}{}", self.config.url, "/api/v1/fees/recommended");
        let resp = client.get(&url).send().await?;
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

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolSpaceConfig {
    #[serde(default = "default_url")]
    pub url: String,
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(default = "default_timeout")]
    pub timeout: std::time::Duration,
    #[serde(default = "default_number_of_retries")]
    pub number_of_retries: u32,
}

impl Default for MempoolSpaceConfig {
    fn default() -> Self {
        Self {
            url: default_url(),
            timeout: default_timeout(),
            number_of_retries: default_number_of_retries(),
        }
    }
}

fn default_url() -> String {
    "https://mempool.tk7.mempool.space".to_string()
}

fn default_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(3)
}

fn default_number_of_retries() -> u32 {
    2
}
