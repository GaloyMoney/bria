use thiserror::Error;

#[derive(Error, Debug)]
pub enum PayoutError {
    #[error("PayoutError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("PayoutError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
