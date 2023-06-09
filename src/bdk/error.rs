use thiserror::Error;

#[derive(Error, Debug)]
pub enum BdkError {
    #[error("BdkError - JoinError: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("BdkError - BdkLibError: {0}")]
    BdkLibError(#[from] bdk::Error),
    #[error("BdkError - ElectrumClient: {0}")]
    ElectrumClient(#[from] electrum_client::Error),
    #[error("BdkError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("BdkError - CouldNotParseIncomingMetadata: {0}")]
    CouldNotParseIncomingMetadata(#[from] serde_json::Error),
}
