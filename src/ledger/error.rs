use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerError {
    #[error("LedgerError - SqlxLedger: {0}")]
    SqlxLedger(#[from] sqlx_ledger::SqlxLedgerError),
    #[error("LedgerError - SerdeJson: {0}")]
    SerdeJson(#[from] serde_json::Error),
}
