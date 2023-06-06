use thiserror::Error;

use super::JobExecutionError;
use crate::{
    account::error::AccountError, address::error::AddressError, bdk::error::BdkError,
    ledger::error::LedgerError, payout::error::PayoutError, payout_queue::error::PayoutQueueError,
    profile::error::ProfileError, wallet::error::WalletError, xpub::error::XPubError,
};

#[derive(Error, Debug)]
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
    XPubError(#[from] XPubError),
    #[error("JobError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("{0}")]
    AccountError(#[from] AccountError),
}

impl JobExecutionError for JobError {}
