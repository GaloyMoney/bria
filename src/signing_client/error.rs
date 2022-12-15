use thiserror::Error;

#[derive(Error, Debug)]
pub enum SigningClientError {
    #[error("SigningClientError - CouldNotConnect: {0}")]
    CouldNotConnect(String),
    #[error("SigningClientError - RemoteCallFailure: {0}")]
    RemoteCallFailure(String),
    #[error("SigningClientError - EncodeError: {0}")]
    EncodeError(#[from] bitcoin::consensus::encode::Error),
}
