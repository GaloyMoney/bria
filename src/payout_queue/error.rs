use thiserror::Error;

#[derive(Error, Debug)]
pub enum PayoutQueueError {
    #[error("PayoutQueueError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("PayoutQueueError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
    #[error("PayoutQueueError - CursorDestructureError: {0}")]
    CursorDestructureError(#[from] es_entity::CursorDestructureError),
}

es_entity::from_es_entity_error!(PayoutQueueError);
