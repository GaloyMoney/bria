use thiserror::Error;

#[derive(Error, Debug)]
pub enum AddressError {
    #[error("AddressError - external_id already exists")]
    ExternalIdAlreadyExists,
    #[error("AddressError - Sqlx: {0}")]
    Sqlx(sqlx::Error),
    #[error("ProfileError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
    #[error("ProfileError - CursorDestructureError: {0}")]
    CursorDestructureError(#[from] es_entity::CursorDestructureError),
}

es_entity::from_es_entity_error!(AddressError);

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
