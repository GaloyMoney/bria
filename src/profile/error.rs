use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("ProfileError - api key does not exist")]
    ProfileKeyNotFound,
    #[error("ProfileError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("ProfileError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
