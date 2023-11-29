use thiserror::Error;

use crate::{
    address::error::AddressError, batch_inclusion::error::BatchInclusionError,
    payout::error::PayoutError,
};

#[derive(Error, Debug)]
pub enum OutboxError {
    #[error("OutboxError - SendEventError")]
    SendEventError,
    #[error("OutboxError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("OutboxError - CouldNotParseIncomingMetadata: {0}")]
    CouldNotParseIncomingMetadata(#[from] serde_json::Error),
    #[error("{0}")]
    PayoutError(#[from] PayoutError),
    #[error("{0}")]
    BatchInclusionError(#[from] BatchInclusionError),
    #[error("{0}")]
    AddressError(#[from] AddressError),
}
