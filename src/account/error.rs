use thiserror::Error;

#[derive(Error, Debug)]
pub enum AccountError {
    #[error("WalletError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("WalletError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
