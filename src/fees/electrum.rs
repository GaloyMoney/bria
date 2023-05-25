use bdk::FeeRate;
use electrum_client::{Client, ConfigBuilder, ElectrumApi};

use crate::{error::*, primitives::TxPriority};
pub struct ElectrumFeeEstimator {
    url: String,
}

impl ElectrumFeeEstimator {
    pub fn new(electrum_url: String) -> Self {
        Self { url: electrum_url }
    }

    pub async fn fee_rate(&self, priority: TxPriority) -> Result<FeeRate, BriaError> {
        let n_blocks = priority.n_blocks();
        let client = Client::from_config(
            &self.url,
            ConfigBuilder::new()
                .retry(10)
                .timeout(Some(4))
                .expect("couldn't set electrum timeout")
                .build(),
        )?;
        let btc_per_kb = client.estimate_fee(n_blocks)?;
        Ok(FeeRate::from_btc_per_kvb(btc_per_kb as f32))
    }
}
