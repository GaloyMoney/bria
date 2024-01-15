use thiserror::Error;

use crate::{
    account::error::AccountError, app::error::ApplicationError, ledger::error::LedgerError,
    profile::error::ProfileError,
};

#[allow(clippy::large_enum_variant)]
#[derive(Error, Debug)]
pub enum AdminApiError {
    #[error("AdminApiError - TonicError: {0}")]
    TonicError(#[from] tonic::transport::Error),
    #[error("AdminApiError - SqlxError: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("AdminApiError - BriaError: {0}")]
    BriaError(ApplicationError),
    #[error("AdminApiError - BadNetworkForDev")]
    BadNetworkForDev,
    #[error("{0}")]
    AccountError(#[from] AccountError),
    #[error("{0}")]
    ProfileError(#[from] ProfileError),
    #[error("{0}")]
    LedgerError(#[from] LedgerError),
    #[error("AdminApiError - DevBootstrapError: {0}")]
    DevBootstrapError(#[from] anyhow::Error),
}

impl From<ApplicationError> for AdminApiError {
    fn from(err: ApplicationError) -> Self {
        match err {
            ApplicationError::Sqlx(e) => AdminApiError::SqlxError(e),
            e => AdminApiError::BriaError(e),
        }
    }
}
