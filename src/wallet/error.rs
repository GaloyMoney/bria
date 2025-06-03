use thiserror::Error;
use es_entity::EsEntityError;
use es_entity::CursorDestructureError;
use serde_json::Error as SerdeJsonError;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("WalletError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("WalletError - EsEntityError: {0}")]
    EsEntityError(#[from] EsEntityError),
    #[error("WalletError - CursorDestructureError: {0}")]
    CursorDestructureError(#[from] CursorDestructureError),
    #[error("WalletError - UnsupportedPubKeyType")]
    UnsupportedPubKeyType,
    #[error("WalletError - BdkMiniscriptError: {0}")]
    BdkMiniscriptError(#[from] bdk::miniscript::Error),
    #[error("WalletError - Submitted Psbt does not have valid signatures.")]
    PsbtDoesNotHaveValidSignatures,
    #[error("WalletError - Unsigned txn in signed and unsigned psbt don't match")]
    UnsignedTxnMismatch,
    #[error("WalletError - SerdeJson: {0}")]
    SerdeJson(#[from] SerdeJsonError),
}