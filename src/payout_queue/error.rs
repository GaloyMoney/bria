use thiserror::Error;

#[derive(Error, Debug)]
pub enum PayoutQueueError {
    #[error("PayoutQueueError - Could not find payout queue with name: {0}")]
    PayoutQueueNameNotFound(String),
    #[error("PayoutQueueError - Could not find payout queue with id: {0}")]
    PayoutQueueIdNotFound(String),
    #[error("PayoutQueueError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("PayoutQueueError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
