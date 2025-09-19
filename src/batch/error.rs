use thiserror::Error;

#[derive(Debug, Error)]
pub enum BatchError {
    #[error("BatchError - Could not find batch with id: {0}")]
    BatchIdNotFound(String),
    #[error("BatchError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("BatchError - BitcoinConsensusEncodeError: {0}")]
    BitcoinConsensusEncodeError(#[from] crate::primitives::bitcoin::consensus::encode::Error),
    #[error("BatchError - Could not deserialize PSBT: {0}")]
    PsbtDeserializationError(#[from] crate::primitives::bitcoin::psbt::Error),
    #[error("BatchError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
}

es_entity::from_es_entity_error!(BatchError);
