use thiserror::Error;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("WalletError - Could not find wallet with name: {0}")]
    WalletNameNotFound(String),
    #[error("WalletError - Could not find wallet with id: {0}")]
    WalletIdNotFound(String),
    #[error("WalletError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("WalletError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
    #[error("WalletError - UnsupportedPubKeyType")]
    UnsupportedPubKeyType,
    #[error("WalletError - BdkMiniscriptError: {0}")]
    BdkMiniscriptError(#[from] bdk::miniscript::Error),
}
