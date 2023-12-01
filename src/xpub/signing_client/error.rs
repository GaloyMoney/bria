use thiserror::Error;

use crate::primitives::bitcoin;

#[derive(Error, Debug)]
pub enum SigningClientError {
    #[error("SigningClientError - CouldNotConnect: {0}")]
    CouldNotConnect(String),
    #[error("SigningClientError - RemoteCallFailure: {0}")]
    RemoteCallFailure(String),
    #[error("SigningClientError - Decode: {0}")]
    Decode(#[from] base64::DecodeError),
    #[error("SigningClientError - HexDecode: {0}")]
    HexConvert(String),
    #[error("SigningClientError - IO: {0}")]
    IO(#[from] std::io::Error),
    #[error("SigningClientError - Deserialization Error: {0}")]
    DeserializationError(#[from] bitcoin::psbt::Error),
}
