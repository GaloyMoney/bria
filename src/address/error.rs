use thiserror::Error;

#[derive(Error, Debug)]
pub enum AddressError {
    #[error("AddressError - external_id already exists")]
    ExternalIdAlreadyExists,
    #[error("AddressError - external_id does not exist")]
    ExternalIdDoesNotExist,
    #[error("AddressError - Sqlx: {0}")]
    Sqlx(sqlx::Error),
    #[error("AddressError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}

impl From<sqlx::Error> for AddressError {
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
