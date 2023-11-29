use thiserror::Error;

use crate::{job::error::JobError, payout_queue::error::PayoutQueueError};

#[derive(Error, Debug)]
pub enum BatchInclusionError {
    #[error("BatchInclusionError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("{0}")]
    JobError(#[from] JobError),
    #[error("{0}")]
    PayoutQueueError(#[from] PayoutQueueError),
}
