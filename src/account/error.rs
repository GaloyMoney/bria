use thiserror::Error;

#[derive(Error, Debug)]
pub enum AccountError {
    #[error("AccountError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("AccountError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
}

es_entity::from_es_entity_error!(AccountError);
