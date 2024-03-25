use thiserror::Error;

#[derive(Error, Debug)]
pub enum PayoutError {
    #[error("PayoutError - Sqlx: {0}")]
    Sqlx(sqlx::Error),
    #[error("PayoutError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
    #[error("PayoutError - Could not find payout with id: {0}")]
    PayoutIdNotFound(String),
    #[error("PayoutError - External Id does not exists")]
    ExternalIdNotFound,
    #[error("PayoutError - Payout is already committed to batch")]
    PayoutAlreadyCommitted,
    #[error("PayoutError - Payout is already cancelled")]
    PayoutAlreadyCancelled,
    #[error("PayoutError - external_id already exists")]
    ExternalIdAlreadyExists,
}

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
