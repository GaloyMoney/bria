use thiserror::Error;

#[derive(Error, Debug)]
pub enum PayoutError {
    #[error("PayoutError - Sqlx: {0}")]
    Sqlx(sqlx::Error),
    #[error("PayoutError - Payout is already committed to batch")]
    PayoutAlreadyCommitted,
    #[error("PayoutError - Payout is already cancelled")]
    PayoutAlreadyCancelled,
    #[error("PayoutError - external_id already exists")]
    ExternalIdAlreadyExists,
    #[error("PayoutError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
    #[error("PayoutError - CursorDestructureError: {0}")]
    CursorDestructureError(#[from] es_entity::CursorDestructureError),
}

es_entity::from_es_entity_error!(PayoutError);

impl From<sqlx::Error> for PayoutError {
    fn from(error: sqlx::Error) -> Self {
        if let Some(err) = error.as_database_error() {
            if let Some(constraint) = err.constraint() {
                if constraint.contains("external_id") {
                    return Self::ExternalIdAlreadyExists;
                }
            }
        }
        Self::Sqlx(error)
    }
}
