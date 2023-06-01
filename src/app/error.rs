use thiserror::Error;

use crate::{
    address::error::AddressError, payout::error::PayoutError, primitives::InternalError,
    wallet::error::WalletError,
};

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("{0}")]
    Internal(#[from] InternalError),
    #[error("{0}")]
    WalletError(#[from] WalletError),
    #[error("{0}")]
    PayoutError(#[from] PayoutError),
    #[error("{0}")]
    AddressError(#[from] AddressError),
}
