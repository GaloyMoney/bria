use thiserror::Error;

#[derive(Error, Debug)]
pub enum BriaError {
    #[error("BriaError - TonicError: {0}")]
    TonicError(#[from] tonic::transport::Error),
    #[error("BriaError - SqlxError: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("BriaError - ParseidError: {0}")]
    ParseIdError(#[from] uuid::Error),
    #[error("BriaError - SqlxLedgerError: {0}")]
    SqlxLedgerError(#[from] sqlx_ledger::SqlxLedgerError),
    #[error("BriaError - SerdeJsonError: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
}
