use thiserror::Error;

#[derive(Debug, Error)]
pub enum DescriptorError {
    #[error("DescriptorError - DescriptorAlreadyInUse")] // Map this in convert ?
    DescriptorAlreadyInUse,
    #[error("DescriptorError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("DescriptorError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
}
