use thiserror::Error;

#[derive(Debug, Error)]
pub enum UtxoError {
    #[error("UtxoError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("UtxoError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
    #[error("UtxoError - Utxo already settled")]
    UtxoAlreadySettledError,
    #[error("UtxoError - Utxo does not exist")]
    UtxoDoesNotExistError,
}

es_entity::from_es_entity_error!(UtxoError);
