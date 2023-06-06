use sqlx_ledger::SqlxLedgerError;
use thiserror::Error;

use crate::{account::error::AccountError, error::*};

#[allow(clippy::large_enum_variant)]
#[derive(Error, Debug)]
pub enum AdminApiError {
    #[error("AdminApiError - TonicError: {0}")]
    TonicError(#[from] tonic::transport::Error),
    #[error("AdminApiError - SqlxError: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("AdminApiError - SqlxLedgerError: {0}")]
    SqlxLedgerError(#[from] SqlxLedgerError),
    #[error("AdminApiError - BriaError: {0}")]
    BriaError(BriaError),
    #[error("AdminApiError - BadNetworkForDev")]
    BadNetworkForDev,
    #[error("{0}")]
    AccountError(#[from] AccountError),
}

impl From<BriaError> for AdminApiError {
    fn from(err: BriaError) -> Self {
        match err {
            BriaError::Sqlx(e) => AdminApiError::SqlxError(e),
            BriaError::Tonic(e) => AdminApiError::TonicError(e),
            e => AdminApiError::BriaError(e),
        }
    }
}
