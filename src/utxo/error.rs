use thiserror::Error;

#[derive(Debug, Error)]
pub enum UtxoError {
    #[error("UtxoError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("UtxoError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
    #[error("UtxoError - Utxo already settled")]
    UtxoAlreadySettledError,
    #[error("UtxoError - Utxo does not exist")]
    UtxoDoesNotExistError,
}
