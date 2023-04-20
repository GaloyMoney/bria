use bdk::blockchain::{Blockchain, ElectrumBlockchain};
use electrum_client::{Client, ConfigBuilder};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    app::BlockchainConfig, batch::*, error::*, ledger::*, primitives::*, signing_session::*,
    wallet::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFinalizingData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
}

#[instrument(
    name = "job.batch_finalizing",
    skip(_pool, signing_sessions, _wallets, _batches, _ledger),
    err
)]
pub async fn execute(
    _pool: sqlx::PgPool,
    data: BatchFinalizingData,
    blockchain_cfg: BlockchainConfig,
    signing_sessions: SigningSessions,
    _ledger: Ledger,
    _wallets: Wallets,
    _batches: Batches,
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
