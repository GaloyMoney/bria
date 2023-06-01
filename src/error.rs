use thiserror::Error;

use crate::{
    address::error::AddressError,
    job::JobExecutionError,
    payout::error::PayoutError,
    primitives::{
        bitcoin::{bip32, consensus, psbt, AddressError as BitcoinAddressError},
        InternalError,
    },
    wallet::error::WalletError,
    xpub::SigningClientError,
};

#[derive(Error, Debug)]
pub enum BriaError {
    #[error("BriaError - Internal: {0}")]
    Internal(#[from] InternalError),
    #[error("BriaError - WalletError: {0}")]
    WalletError(#[from] WalletError),
    #[error("BriaError - PayoutError: {0}")]
    PayoutError(#[from] PayoutError),
    #[error("BriaError - AddressError: {0}")]
    AddressError(#[from] AddressError),

    #[error("BriaError - FromHex: {0}")]
    FromHex(#[from] hex::FromHexError),
    #[error("BriaError - Tonic: {0}")]
    Tonic(#[from] tonic::transport::Error),
    #[error("BriaError - Sqlx: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("BriaError - Migrate: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("BriaError - ParseId: {0}")]
    ParseId(#[from] uuid::Error),
    #[error("BriaError - SqlxLedger: {0}")]
    SqlxLedger(#[from] sqlx_ledger::SqlxLedgerError),
    #[error("BriaError - SerdeJson: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("BriaError - psbt::Error: {0}")]
    PsbtError(#[from] psbt::Error),
    #[error("BriaError - EventStreamError: {0}")]
    EventStreamError(#[from] tokio_stream::wrappers::errors::BroadcastStreamRecvError),
    #[error("BriaError - SendEventError")]
    SendEventError,
    #[error("BriaError - MissingTxMetadata")]
    MissingTxMetadata,
    #[error("BriaError - SigningClientError: {0}")]
    SigningClient(#[from] SigningClientError),
    #[error("BriaError - EntityError: {0}")]
    EntityError(#[from] crate::entity::EntityError),
    #[error("BriaError - Bip32: {0}")]
    Bip32(#[from] bip32::Error),
    #[error("BriaError - WalletNotFound")]
    WalletNotFound,
    #[error("BriaError - PayoutNotFound")]
    PayoutNotFound,
    #[error("BriaError - ProfileNotFound")]
    ProfileNotFound,
    #[error("BriaError - BatchSigningSessionNotFound")]
    BatchSigningSessionNotFound,
    #[error("BriaError - CouldNotRetrieveWalletBalance")]
    CouldNotRetrieveWalletBalance,
    #[error("BriaError - PayoutQueueNotFound({0})")]
    PayoutQueueNotFound(String),
    #[error("BriaError - BatchNotFound")]
    BatchNotFound,
    #[error("BriaError - PsbtMissingInSigningSessions")]
    PsbtMissingInSigningSessions,
    #[error("BriaError - DescriptorAlreadyInUse")]
    DescriptorAlreadyInUse,
    #[error("BriaError - BitcoinConsensusEncodeError: {0}")]
    BitcoinConsensusEncodeError(#[from] consensus::encode::Error),
    #[error("BriaError - TryFromIntError")]
    TryFromIntError(#[from] std::num::TryFromIntError),
    #[error("BriaError - BitcoinAddressParseError")]
    BitcoinAddressParseError(#[from] BitcoinAddressError),
    #[error("BriaError - XPubDepthMismatch: expected depth {0}, got {1}")]
    XPubDepthMismatch(u8, usize),
    #[error("BriaError - XPubParseError: {0}")]
    XPubParseError(bdk::bitcoin::util::base58::Error),
    #[error("BriaError - UnsupportedPubKeyType")]
    UnsupportedPubKeyType,
    #[error("BriaError - BdkMiniscriptError: {0}")]
    BdkMiniscriptError(#[from] bdk::miniscript::Error),
    #[error("BriaError - FeeEstimation: {0}")]
    FeeEstimation(reqwest::Error),
    #[error("BriaError - CouldNotCombinePsbts: {0}")]
    CouldNotCombinePsbts(psbt::Error),
    #[error("BriaError - CouldNotParseIncomingMetadata: {0}")]
    CouldNotParseIncomingMetadata(serde_json::Error),
    #[error("BriaError - CouldNotParseIncomingUuid: {0}")]
    CouldNotParseIncomingUuid(uuid::Error),
}

impl JobExecutionError for BriaError {}
