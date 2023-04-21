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
    name = "job.batch_signing",
    skip(pool, wallets, signing_sessions, batches, xpubs),
    fields(stalled),
    err
)]
pub async fn execute(
    pool: sqlx::PgPool,
    data: BatchSigningData,
    blockchain_cfg: BlockchainConfig,
    batches: Batches,
    signing_sessions: SigningSessions,
    wallets: Wallets,
    xpubs: XPubs,
) -> Result<(BatchSigningData, bool), BriaError> {
    let mut stalled = false;
    let mut last_err = None;
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

    let mut any_updated = false;
    for (xpub_id, session) in sessions.iter_mut().filter(|(_, s)| !s.is_completed()) {
        any_updated = true;
        let account_xpub = if let Some(xpub) = account_xpub_cache.remove(xpub_id) {
            xpub
        } else {
            xpubs
                .find_from_ref(data.account_id, xpub_id.to_string())
                .await?
        };
        let mut client = match account_xpub.remote_signing_client().await {
            Ok(Some(client)) => client,
            Ok(None) => {
                session.attempt_failed(SigningFailureReason::SignerConfigMissing);
                stalled = true;
                tracing::warn!("signer_config_missing");
                continue;
            }
            Err(err) => {
                session.attempt_failed(&err);
                tracing::error!("{}", err.to_string());
                last_err = Some(err);
                continue;
            }
        };
        match client.sign_psbt(&session.unsigned_psbt).await {
            Ok(psbt) => {
                session.remote_signing_complete(psbt);
            }
            Err(err) => {
                session.attempt_failed(&err);
                tracing::error!("{}", err.to_string());
                last_err = Some(err);
                continue;
            }
        }
    }

    if any_updated {
        let mut tx = pool.begin().await?;
        signing_sessions.update_sessions(&mut tx, &sessions).await?;
        tx.commit().await?;
    }

    tracing::Span::current().record("stalled", stalled);
    if let Some(err) = last_err {
        return Err(err.into());
    } else if stalled {
        return Ok((data, false));
    }

    let mut sessions = sessions.into_values();
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

    let tx = first_psbt.extract_tx();
    batches.set_signed_tx(data.batch_id, tx).await?;

    Ok((data, true))
}
