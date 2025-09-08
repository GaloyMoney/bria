use thiserror::Error;

#[derive(Debug, Error)]
pub enum DescriptorError {
    #[error("DescriptorError - DescriptorAlreadyInUse")] // Map this in convert ?
    DescriptorAlreadyInUse,
    #[error("DescriptorError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("DescriptorError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
}

es_entity::from_es_entity_error!(DescriptorError);
