use thiserror::Error;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("WalletError - Could not find wallet with name: {0}")]
    WalletNameNotFound(String),
    #[error("WalletError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("WalletError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
