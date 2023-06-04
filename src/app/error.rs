use thiserror::Error;

use crate::{
    address::error::AddressError, bdk::error::BdkError, payout::error::PayoutError,
    payout_queue::error::PayoutQueueError, profile::error::ProfileError,
    wallet::error::WalletError,
};

#[derive(Error, Debug)]
pub enum ApplicationError {
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
}
