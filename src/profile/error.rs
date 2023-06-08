use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("ProfileError - Api key does not exist")]
    ProfileKeyNotFound,
    #[error("ProfileError - Could not find profile with name: {0}")]
    ProfileNameNotFound(String),
    #[error("ProfileError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("ProfileError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
