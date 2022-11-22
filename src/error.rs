use thiserror::Error;

#[derive(Error, Debug)]
pub enum BriaError {
    #[error("BriaError - Tonic: {0}")]
    Tonic(#[from] tonic::transport::Error),
    #[error("BriaError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("BriaError - ParseId: {0}")]
    ParseId(#[from] uuid::Error),
    #[error("BriaError - SqlxLedger: {0}")]
    SqlxLedger(#[from] sqlx_ledger::SqlxLedgerError),
    #[error("BriaError - SerdeJson: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("BriaError - Bip32: {0}")]
    Bip32(#[from] bitcoin::util::bip32::Error),
    #[error("BriaError - WalletNotFound")]
    WalletNotFound,
    #[error("BriaError - XPubDepthMissmatch: expected depth {0}, got {1}")]
    XPubDepthMissmatch(u8, usize),
    #[error("BriaError - JoinError: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}
