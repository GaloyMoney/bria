use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerError {
    #[error("LedgerError - SqlxLedger: {0}")]
    SqlxLedger(#[from] sqlx_ledger::SqlxLedgerError),
    #[error("LedgerError - SerdeJson: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("LedgerError - EventStreamError: {0}")]
    EventStreamError(#[from] tokio_stream::wrappers::errors::BroadcastStreamRecvError),
    #[error("LedgerError - MissingTxMetadata")]
    MissingTxMetadata,
    #[error("LedgerError - MismatchedTxMetadata: {0}")]
    MismatchedTxMetadata(serde_json::Error),
    #[error("LedgerError - NotFound: {0}")]
    ExpectedEntryNotFoundInTx(&'static str),
    #[error("LedgerError - Transaction not found")]
    TransactionNotFound,
}
