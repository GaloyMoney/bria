use thiserror::Error;

use crate::payout_queue::error::PayoutQueueError;

#[derive(Error, Debug)]
pub enum BatchInclusionError {
    #[error("BatchInclusionError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("{0}")]
    PayoutQueueError(#[from] PayoutQueueError),
}
