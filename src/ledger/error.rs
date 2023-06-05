use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerError {
    #[error("BriaError - SqlxLedger: {0}")]
    SqlxLedger(#[from] sqlx_ledger::SqlxLedgerError),
}
