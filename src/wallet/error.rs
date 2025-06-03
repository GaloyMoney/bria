use thiserror::Error;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("WalletError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("WalletError - EsEntityError: {0}")]
    EsEntityError(#[from] es_entity::EsEntityError),
    #[error("WalletError - CursorDestructureError: {0}")]
    CursorDestructureError(#[from] es_entity::CursorDestructureError),
    #[error("WalletError - UnsupportedPubKeyType")]
    UnsupportedPubKeyType,
    #[error("WalletError - BdkMiniscriptError: {0}")]
    BdkMiniscriptError(#[from] bdk::miniscript::Error),
    #[error("WalletError - Submitted Psbt does not have valid signatures.")]
    PsbtDoesNotHaveValidSignatures,
    #[error("WalletError - Unsigned txn in signed and unsigned psbt don't match")]
    UnsignedTxnMismatch,
}