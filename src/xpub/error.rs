use thiserror::Error;

use crate::api::proto::Xpub;

#[derive(Error, Debug)]
pub enum XpubError {
    #[error("XpubError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("XpubError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
    #[error("XpubError - CouldNotParseIncomingMetadata: {0}")]
    CouldNotParseIncomingMetadata(#[from] serde_json::Error),
    #[error("XpubError - FromHex: {0}")]
    FromHex(#[from] hex::FromHexError),
    #[error("XpubError - XpubDepthMismatch: expected depth {0}, got {1}")]
    XPubDepthMismatch(u8, usize),
    #[error("XpubError - XpubParseError: {0}")]
    XPubParseError(bdk::bitcoin::base58::Error),
    #[error("XpubError - Bip32: {0}")]
    Bip32(#[from] crate::primitives::bitcoin::bip32::Error),
    #[error("XpubError - UnsupportedPubKeyType")]
    UnsupportedPubKeyType,
    #[error("Could not decrypt signer config: {0}")]
    CouldNotDecryptSignerConfig(chacha20poly1305::Error),
    #[error("XpubError - EsEntityError: {0}")]
    EsEntityError(es_entity::EsEntityError),
    #[error("XpubError - CursorDestructureError: {0}")]
    CursorDestructureError(#[from] es_entity::CursorDestructureError),
}

es_entity::from_es_entity_error!(XpubError);
