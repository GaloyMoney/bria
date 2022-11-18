use thiserror::Error;

#[allow(clippy::large_enum_variant)]
#[derive(Error, Debug)]
pub enum AdminApiError {
    #[error("AdminApiError - TonicError: {0}")]
    TonicError(#[from] tonic::transport::Error),
}
