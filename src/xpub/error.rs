use thiserror::Error;

#[derive(Error, Debug)]
pub enum XPubError {
    #[error("XPubError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("XPubError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
