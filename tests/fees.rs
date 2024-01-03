use bria::{fees::*, primitives::TxPriority};

#[tokio::test]
async fn mempool_space() -> anyhow::Result<()> {
    let mempool_space_config = MempoolSpaceConfig::default();
    let mempool_space = MempoolSpaceClient::new(mempool_space_config);
    let fee_rate = mempool_space.fee_rate(TxPriority::NextBlock).await?;
    assert!(fee_rate.as_sat_per_vb() > 0.0);
    Ok(())
}

#[tokio::test]
async fn blockstream() -> anyhow::Result<()> {
    let blockstream_config = BlockstreamConfig::default();
    let blockstream = BlockstreamClient::new(blockstream_config);
    let fee_rate = blockstream.fee_rate(TxPriority::NextBlock).await?;
    assert!(fee_rate.as_sat_per_vb() > 0.0);
    Ok(())
}
