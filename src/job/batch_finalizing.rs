use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use electrum_client::{Client, ConfigBuilder};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{app::BlockchainConfig, error::*, primitives::*, signing_session::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFinalizingData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
}

#[instrument(name = "job.batch_finalizing", skip(signing_sessions), err)]
pub async fn execute(
    data: BatchFinalizingData,
    blockchain_cfg: BlockchainConfig,
    signing_sessions: SigningSessions,
) -> Result<BatchFinalizingData, BriaError> {
    let blockchain = init_electrum(&blockchain_cfg.electrum_url).await?;
    let sessions = signing_sessions
        .find_for_batch(data.account_id, data.batch_id)
        .await?
        .ok_or(BriaError::BatchSigningSessionNotFound)?;
    let mut sessions = sessions.xpub_sessions.into_values();
    let mut first_psbt = sessions
        .next()
        .and_then(|s| s.signed_psbt().cloned())
        .ok_or(BriaError::PsbtMissingInSigningSessions)?;
    for s in sessions {
        first_psbt.combine(
            s.signed_psbt()
                .ok_or(BriaError::PsbtMissingInSigningSessions)?
                .clone(),
        )?;
    }
    blockchain.broadcast(&first_psbt.extract_tx())?;
    Ok(data)
}

async fn init_electrum(electrum_url: &str) -> Result<ElectrumBlockchain, BriaError> {
    let blockchain = ElectrumBlockchain::from(Client::from_config(
        electrum_url,
        ConfigBuilder::new()
            .retry(10)
            .timeout(Some(4))
            .expect("couldn't set electrum timeout")
            .build(),
    )?);
    Ok(blockchain)
}
