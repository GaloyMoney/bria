use thiserror::Error;

use super::JobExecutionError;
use crate::{
    account::error::AccountError,
    address::error::AddressError,
    batch::error::BatchError,
    bdk::error::BdkError,
    fees::error::FeeEstimationError,
    ledger::error::LedgerError,
    outbox::error::OutboxError,
    payout::error::PayoutError,
    payout_queue::error::PayoutQueueError,
    primitives::bitcoin::psbt,
    profile::error::ProfileError,
    signing_session::error::SigningSessionError,
    utxo::error::UtxoError,
    wallet::error::WalletError,
    xpub::{error::XpubError, SigningClientError},
};

#[derive(Error, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum JobError {
    #[error("{0}")]
    BdkError(#[from] BdkError),
    #[error("{0}")]
    WalletError(#[from] WalletError),
    #[error("{0}")]
    PayoutError(#[from] PayoutError),
    #[error("{0}")]
    AddressError(#[from] AddressError),
    #[error("{0}")]
    ProfileError(#[from] ProfileError),
    #[error("{0}")]
    PayoutQueueError(#[from] PayoutQueueError),
    #[error("{0}")]
    LedgerError(#[from] LedgerError),
    #[error("{0}")]
    XPubError(#[from] XpubError),
    #[error("{0}")]
    UtxoError(#[from] UtxoError),
    #[error("{0}")]
    FeeEstimationError(#[from] FeeEstimationError),
    #[error("{0}")]
    BatchError(#[from] BatchError),
    #[error("{0}")]
    SigningSessionError(#[from] SigningSessionError),
    #[error("{0}")]
    AccountError(#[from] AccountError),
    #[error("{0}")]
    OutboxError(#[from] OutboxError),
    #[error("{0}")]
    SigningClientError(#[from] SigningClientError),
    #[error("JobError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("JobError - PsbtMissingInSigningSessions")]
    PsbtMissingInSigningSessions,
    #[error("JobError - psbt::Error: {0}")]
    PsbtError(#[from] psbt::Error),
}

impl JobExecutionError for JobError {}
