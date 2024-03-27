use thiserror::Error;

use crate::primitives::ProfileId;

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error("ProfileError - Api key does not exist")]
    ProfileKeyNotFound,
    #[error("ProfileError - Could not find profile with name: {0}")]
    ProfileNameNotFound(String),
    #[error("ProfileError - Could not find profile with id: {0}")]
    ProfileIdNotFound(ProfileId),
    #[error("ProfileError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("ProfileError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
