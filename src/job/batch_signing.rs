use serde::{Deserialize, Serialize};
use tracing::instrument;

use std::collections::HashMap;

use crate::{
    app::BlockchainConfig, batch::*, error::*, primitives::*, signing_session::*, wallet::*,
    xpub::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSigningData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
    #[serde(flatten)]
    pub(super) tracing_data: HashMap<String, String>,
}

#[instrument(
    name = "job.batch_wallet_signing",
    skip(_pool, wallets, signing_sessions, batches, xpubs),
    err
)]
pub async fn execute(
    _pool: sqlx::PgPool,
    data: BatchSigningData,
    blockchain_cfg: BlockchainConfig,
    batches: Batches,
    signing_sessions: SigningSessions,
    wallets: Wallets,
    xpubs: XPubs,
) -> Result<BatchSigningData, BriaError> {
    let mut last_err = None;
    let any_non_stalled_error = false;
    let (mut sessions, mut account_xpub_cache) = if let Some(batch_session) = signing_sessions
        .find_for_batch(data.account_id, data.batch_id)
        .await?
    {
        (batch_session.xpub_sessions, HashMap::new())
    } else {
        let mut new_sessions = HashMap::new();
        let mut account_xpubs = HashMap::new();
        let batch = batches.find_by_id(data.account_id, data.batch_id).await?;
        let unsigned_psbt = batch.unsigned_psbt;
        for (wallet_id, keychain_utxos) in batch.included_utxos {
            let wallet = wallets.find_by_id(wallet_id).await?;
            let keychain_xpubs = wallet.xpubs_for_keychains(keychain_utxos.keys());
            for (_, keychain_xpubs) in keychain_xpubs.into_iter() {
                for xpub in keychain_xpubs.into_iter() {
                    let account_xpub = xpubs
                        .find_from_ref(data.account_id, xpub.id().to_string())
                        .await?;
                    let new_session = NewSigningSession::builder()
                        .account_id(data.account_id)
                        .batch_id(data.batch_id)
                        .unsigned_psbt(unsigned_psbt.clone())
                        .build()
                        .expect("Could not build signing session");
                    new_sessions.insert(account_xpub.id(), new_session);
                    account_xpubs.insert(account_xpub.id(), account_xpub);
                }
            }
        }

        (
            signing_sessions
                .persist_new_sessions(new_sessions)
                .await?
                .xpub_sessions,
            account_xpubs,
        )
    };

    for (xpub_id, session) in sessions.iter_mut() {
        let account_xpub = if let Some(xpub) = account_xpub_cache.remove(xpub_id) {
            xpub
        } else {
            xpubs
                .find_from_ref(data.account_id, xpub_id.to_string())
                .await?
        };
        if let Some(_signer) = account_xpub.signer {
            //
        } else if !any_non_stalled_error {
            last_err = Some(BriaError::SigningSessionStalled(
                session.signer_config_missing(),
            ));
        }
    }

    signing_sessions.update_sessions(sessions).await?;

    if let Some(last_err) = last_err {
        Err(last_err)
    } else {
        Ok(data)
    }
}
