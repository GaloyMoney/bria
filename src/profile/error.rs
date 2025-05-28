use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("ProfileError - Api key does not exist")]
    ProfileKeyNotFound,
    #[error("ProfileError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("ProfileError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
    #[error("ProfileError - CursorDestructureError: {0}")]
    CursorDestructureError(#[from] es_entity::CursorDestructureError),
}

es_entity::from_es_entity_error!(ProfileError);
