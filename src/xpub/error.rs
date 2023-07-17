use thiserror::Error;

#[derive(Error, Debug)]
pub enum XPubError {
    #[error("XPubError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("XPubError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
    #[error("XPubError - CouldNotParseIncomingMetadata: {0}")]
    CouldNotParseIncomingMetadata(#[from] serde_json::Error),
    #[error("XPubError - FromHex: {0}")]
    FromHex(#[from] hex::FromHexError),
    #[error("XPubError - XPubDepthMismatch: expected depth {0}, got {1}")]
    XPubDepthMismatch(u8, usize),
    #[error("XPubError - XPubParseError: {0}")]
    XPubParseError(bdk::bitcoin::util::base58::Error),
    #[error("XPubError - Bip32: {0}")]
    Bip32(#[from] crate::primitives::bitcoin::bip32::Error),
    #[error("XPubError - UnsupportedPubKeyType")]
    UnsupportedPubKeyType,
    #[error("Could not decrypt signer config: {0}")]
    CouldNotDecryptSignerConfig(chacha20poly1305::Error),
}
