use thiserror::Error;

#[derive(Error, Debug)]
pub enum SigningSessionError {
    #[error("SigningSessionError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("SigningSessionError - EsEntityError: {0})")]
    EsEntityError(es_entity::EsEntityError),
    #[error("SigningSessionError - CursorDestructureError: {0}")]
    CursorDestructureError(#[from] es_entity::CursorDestructureError),
}
es_entity::from_es_entity_error!(SigningSessionError);
