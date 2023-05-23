use bria::{fee_estimation::*, primitives::TxPriority};

#[tokio::test]
async fn mempool_space() -> anyhow::Result<()> {
    let url = "https://mempool.space/api/v1/fees/recommended".to_string();
    let rate = MempoolSpaceClient::fee_rate(url, TxPriority::NextBlock).await?;
    assert!(rate.as_sat_per_vb() > 0.0);
    Ok(())
}
