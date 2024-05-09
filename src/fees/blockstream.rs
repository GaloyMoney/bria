use bdk::FeeRate;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::error::FeeEstimationError;
use crate::primitives::TxPriority;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeeEstimatesResponse {
    #[serde(rename = "1")]
    next_block: f32,
    #[serde(rename = "3")]
    half_hour_fee: f32,
    #[serde(rename = "6")]
    hour_fee: f32,
}

#[derive(Clone, Debug)]
pub struct BlockstreamClient {
    config: BlockstreamConfig,
}

impl BlockstreamClient {
    pub fn new(config: BlockstreamConfig) -> Self {
        Self { config }
    }

    #[instrument(name = "blockstream.fee_rate", skip(self), ret, err)]
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

        let url = format!("{}{}", self.config.url, "/api/fee-estimates");
        let resp = client.get(&url).send().await?;
        let fee_estimations = resp
            .json::<FeeEstimatesResponse>()
            .await
            .map_err(FeeEstimationError::CouldNotDecodeResponseBody)?;
        match priority {
            TxPriority::HalfHour => Ok(FeeRate::from_sat_per_vb(fee_estimations.half_hour_fee)),
            TxPriority::OneHour => Ok(FeeRate::from_sat_per_vb(fee_estimations.hour_fee)),
            TxPriority::NextBlock => Ok(FeeRate::from_sat_per_vb(fee_estimations.next_block)),
        }
    }
}

#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockstreamConfig {
    #[serde(default = "default_url")]
    pub url: String,
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    #[serde(default = "default_timeout")]
    pub timeout: std::time::Duration,
    #[serde(default = "default_number_of_retries")]
    pub number_of_retries: u32,
}

impl Default for BlockstreamConfig {
    fn default() -> Self {
        Self {
            url: default_url(),
            timeout: default_timeout(),
            number_of_retries: default_number_of_retries(),
        }
    }
}

fn default_url() -> String {
    "https://blockstream.info".to_string()
}

fn default_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(3)
}

fn default_number_of_retries() -> u32 {
    2
}
