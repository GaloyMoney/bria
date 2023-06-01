use thiserror::Error;

#[derive(Error, Debug)]
pub enum InternalError {
    #[error("InternalError - JoinError: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("InternalError - BdkError: {0}")]
    BdkError(#[from] bdk::Error),
    #[error("InternalError - ElectrumClient: {0}")]
    ElectrumClient(#[from] electrum_client::Error),
}
