use thiserror::Error;

#[allow(clippy::large_enum_variant)]
#[derive(Error, Debug)]
pub enum PayjoinError {
    #[error("PayjoinError - Error")]
    Error,
}
