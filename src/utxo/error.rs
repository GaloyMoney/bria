use thiserror::Error;

#[derive(Debug, Error)]
pub enum UtxoError {
    #[error("UtxoError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("UtxoError - Utxo already settled")]
    UtxoAlreadySettledError,
    #[error("UtxoError - Utxo does not exist")]
    UtxoDoesNotExistError,
    #[error("UtxoError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
}

es_entity::from_es_entity_error!(UtxoError);
