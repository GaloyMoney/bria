use bria::{fee_estimation::*, primitives::TxPriority};

#[tokio::test]
async fn mempool_space() -> anyhow::Result<()> {
    let url = "https://mempool.space/api/v1/fees/recommended".to_string();
    let mempool_space = MempoolSpaceClient::new(url);
    let fee_rate = mempool_space.fee_rate(TxPriority::NextBlock).await?;
    assert!(fee_rate.as_sat_per_vb() > 0.0);
    Ok(())
}
