use serde::{Deserialize, Serialize};
use tracing::instrument;

use std::collections::HashMap;

use super::error::JobError;
use crate::{
    app::BlockchainConfig, batch::*, primitives::*, signing_session::*, wallet::*, xpub::*,
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
    skip(
        pool,
        wallets,
        signing_sessions,
        batches,
        xpubs,
        signer_encryption_config
    ),
    fields(stalled, txid, finalization_status),
    err
)]
#[allow(clippy::too_many_arguments)]
pub async fn execute(
    pool: sqlx::PgPool,
    data: BatchSigningData,
    blockchain_cfg: BlockchainConfig,
    batches: Batches,
    signing_sessions: SigningSessions,
    wallets: Wallets,
    xpubs: XPubs,
    signer_encryption_config: SignerEncryptionConfig,
) -> Result<(BatchSigningData, bool), JobError> {
    let span = tracing::Span::current();
    let mut stalled = false;
    let mut last_err = None;
    let mut current_keychain = None;
    let (mut sessions, mut account_xpub_cache) = if let Some(batch_session) = signing_sessions
        .list_for_batch(data.account_id, data.batch_id)
        .await?
    {
        (batch_session.xpub_sessions, HashMap::new())
    } else {
        let mut new_sessions = HashMap::new();
        let mut account_xpubs = HashMap::new();
        let batch = batches.find_by_id(data.account_id, data.batch_id).await?;
        span.record("tx_id", tracing::field::display(batch.bitcoin_tx_id));
        let unsigned_psbt = batch.unsigned_psbt;
        for (wallet_id, summary) in batch.wallet_summaries {
            let wallet = wallets.find_by_id(wallet_id).await?;
            if current_keychain.is_none() {
                current_keychain = Some(wallet.current_keychain_wallet(&pool));
            }
            let keychain_xpubs = wallet.xpubs_for_keychains(&summary.signing_keychains);
            for (_, keychain_xpubs) in keychain_xpubs.into_iter() {
                for xpub in keychain_xpubs.into_iter() {
                    let account_xpub = xpubs
                        .find_from_ref(data.account_id, xpub.fingerprint())
                        .await?;
                    let new_session = NewSigningSession::builder()
                        .account_id(data.account_id)
                        .batch_id(data.batch_id)
                        .xpub_fingerprint(xpub.fingerprint())
                        .unsigned_psbt(unsigned_psbt.clone())
                        .build()
                        .expect("Could not build signing session");
                    new_sessions.insert(account_xpub.fingerprint(), new_session);
                    account_xpubs.insert(account_xpub.fingerprint(), account_xpub);
                }
            }
        }

        (
            signing_sessions
                .persist_sessions(new_sessions)
                .await?
                .xpub_sessions,
            account_xpubs,
        )
    };

    let mut any_updated = false;
    for (xpub_fingerprint, session) in sessions.iter_mut().filter(|(_, s)| !s.is_completed()) {
        any_updated = true;
        let account_xpub = if let Some(xpub) = account_xpub_cache.remove(xpub_fingerprint) {
            xpub
        } else {
            xpubs
                .find_from_ref(data.account_id, xpub_fingerprint)
                .await?
        };
        let mut client = match account_xpub
            .remote_signing_client(signer_encryption_config.key)
            .await
        {
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
        let mut op = signing_sessions.begin_op().await?;
        signing_sessions.update_sessions(&mut op, &sessions).await?;
        op.commit().await?;
    }
    let mut sessions = sessions.into_values();

    span.record("stalled", tracing::field::display(stalled));
    if let Some(mut first_signed_psbt) = sessions.find_map(|s| s.signed_psbt().cloned()) {
        for s in sessions {
            if let Some(psbt) = s.signed_psbt() {
                let _ = first_signed_psbt.combine(psbt.clone());
            }
        }
        if current_keychain.is_none() {
            let batch = batches.find_by_id(data.account_id, data.batch_id).await?;
            span.record("tx_id", tracing::field::display(batch.bitcoin_tx_id));
            let wallet_id = batch.wallet_summaries.into_keys().next().unwrap();
            let wallet = wallets.find_by_id(wallet_id).await?;
            current_keychain = Some(wallet.current_keychain_wallet(&pool));
        }
        match (
            current_keychain
                .expect("keychain should always exist")
                .finalize_psbt(first_signed_psbt)
                .await,
            last_err,
        ) {
            (Ok(Some(finalized_psbt)), _) => {
                span.record("finalization_status", "complete");
                let tx = finalized_psbt.extract_tx();
                batches.set_signed_tx(data.batch_id, tx).await?;
                Ok((data, true))
            }
            (_, Some(e)) => {
                span.record("finalization_status", "returning_last_error");
                Err(e.into())
            }
            (Ok(None), _) => {
                span.record("finalization_status", "stalled_due_to_finalization");
                Ok((data, false))
            }
            _ if stalled => {
                span.record("finalization_status", "stalled");
                Ok((data, false))
            }
            (Err(err), _) => {
                span.record("finalization_status", "errored");
                Err(err.into())
            }
        }
    } else if let Some(err) = last_err {
        Err(err.into())
    } else {
        Ok((data, false))
    }
}
