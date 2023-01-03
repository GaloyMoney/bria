use bria::{fee_estimation::*, primitives::TxPriority};

#[tokio::test]
async fn mempool_space() -> anyhow::Result<()> {
    let rate = MempoolSpaceClient::fee_rate(TxPriority::NextBlock).await?;
    assert!(rate.as_sat_per_vb() > 0.0);
    Ok(())
}
