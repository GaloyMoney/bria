use thiserror::Error;

#[derive(Error, Debug)]
pub enum BriaError {
    #[error("BriaError - TonicError: {0}")]
    TonicError(#[from] tonic::transport::Error),
    #[error("BriaError - SqlxError: {0}")]
    SqlxError(#[from] sqlx::Error),
}
