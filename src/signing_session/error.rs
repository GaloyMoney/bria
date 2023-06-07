use thiserror::Error;

#[derive(Error, Debug)]
pub enum SigningSessionError {
    #[error("SigningSessionError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("SigningSessionError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
