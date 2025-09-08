use thiserror::Error;

#[derive(Error, Debug)]
pub enum XPubError {
    #[error("XPubError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("XPubError - CouldNotParseIncomingMetadata: {0}")]
    CouldNotParseIncomingMetadata(#[from] serde_json::Error),
    #[error("XPubError - FromHex: {0}")]
    FromHex(#[from] hex::FromHexError),
    #[error("XPubError - XPubDepthMismatch: expected depth {0}, got {1}")]
    XPubDepthMismatch(u8, usize),
    #[error("XPubError - XPubParseError: {0}")]
    XPubParseError(bdk::bitcoin::base58::Error),
    #[error("XPubError - Bip32: {0}")]
    Bip32(#[from] crate::primitives::bitcoin::bip32::Error),
    #[error("XPubError - UnsupportedPubKeyType")]
    UnsupportedPubKeyType,
    #[error("Could not decrypt signer config: {0}")]
    CouldNotDecryptSignerConfig(chacha20poly1305::Error),
    #[error("XPubError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
    #[error("XPubError - CursorDestructureError: {0}")]
    CursorDestructureError(#[from] es_entity::CursorDestructureError),
}

es_entity::from_es_entity_error!(XPubError);
