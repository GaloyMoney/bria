use thiserror::Error;

use crate::job::error::JobError;

#[derive(Error, Debug)]
pub enum BatchInclusionError {
    #[error("BatchInclusionError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("{0}")]
    JobError(#[from] JobError),
}
