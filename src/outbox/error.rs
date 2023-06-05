use thiserror::Error;

#[derive(Error, Debug)]
pub enum OutboxError {
    #[error("OutboxError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
}
