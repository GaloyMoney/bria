use thiserror::Error;

use crate::{payout::error::PayoutError, primitives::InternalError, wallet::error::WalletError};

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("BriaError - Internal: {0}")]
    Internal(#[from] InternalError),
    #[error("BriaError - WalletError: {0}")]
    WalletError(#[from] WalletError),
    #[error("BriaError - PayoutError: {0}")]
    PayoutError(#[from] PayoutError),
}
