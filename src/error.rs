use thiserror::Error;

#[derive(Error, Debug)]
pub enum BriaError {
    #[error("BriaError - SqlxError: {0}")]
    SqlxError(#[from] sqlx::Error),
}
