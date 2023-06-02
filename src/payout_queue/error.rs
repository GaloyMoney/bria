use thiserror::Error;

#[derive(Error, Debug)]
pub enum PayoutQueueError {
    #[error("PayoutQueueError - PayoutQueueNotFound({0})")]
    PayoutQueueNotFound(String),
    #[error("PayoutQueueError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("PayoutQueueError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
