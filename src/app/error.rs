use chacha20poly1305;
use thiserror::Error;

use crate::{
    address::error::AddressError,
    batch::error::BatchError,
    batch_inclusion::error::BatchInclusionError,
    bdk::error::BdkError,
    descriptor::error::DescriptorError,
    fees::error::FeeEstimationError,
    job::error::JobError,
    ledger::error::LedgerError,
    outbox::error::OutboxError,
    payout::error::PayoutError,
    payout_queue::error::PayoutQueueError,
    primitives::{bitcoin, PayoutDestination},
    profile::error::ProfileError,
    signing_session::error::SigningSessionError,
    utxo::error::UtxoError,
    wallet::error::WalletError,
    xpub::error::XPubError,
};

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("{0}")]
    BdkError(#[from] BdkError),
    #[error("{0}")]
    BatchInclusionError(#[from] BatchInclusionError),
    #[error("{0}")]
    WalletError(#[from] WalletError),
    #[error("{0}")]
    PayoutError(#[from] PayoutError),
    #[error("{0}")]
    AddressError(#[from] AddressError),
    #[error("{0}")]
    ProfileError(#[from] ProfileError),
    #[error("{0}")]
    PayoutQueueError(#[from] PayoutQueueError),
    #[error("{0}")]
    LedgerError(#[from] LedgerError),
    #[error("{0}")]
    XPubError(#[from] XPubError),
    #[error("{0}")]
    JobError(#[from] JobError),
    #[error("{0}")]
    OutboxError(#[from] OutboxError),
    #[error("{0}")]
    UtxoError(#[from] UtxoError),
    #[error("{0}")]
    FeeEstimationError(#[from] FeeEstimationError),
    #[error("{0}")]
    BatchError(#[from] BatchError),
    #[error("{0}")]
    SigningSessionError(#[from] SigningSessionError),
    #[error("{0}")]
    DescriptorError(#[from] DescriptorError),
    #[error("{0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("{0}")]
    ServerError(#[from] tonic::transport::Error),
    #[error("UnsupportedPubKeyType")]
    UnsupportedPubKeyType,
    #[error("CouldNotParseIncomingMetadata: {0}")]
    CouldNotParseIncomingMetadata(serde_json::Error),
    #[error("CouldNotParseIncomingUuid: {0}")]
    CouldNotParseIncomingUuid(uuid::Error),
    #[error("DestinationBlocked - sending to '{0}' is prohibited")]
    DestinationBlocked(PayoutDestination),
    #[error("DestinationNotAllowed - profile is not allowed to send to '{0}'")]
    DestinationNotAllowed(PayoutDestination),
    #[error("Signing Session not found for batch id: {0}")]
    SigningSessionNotFoundForBatchId(crate::primitives::BatchId),
    #[error("Signing Session not found for xpub id: {0}")]
    SigningSessionNotFoundForXPubId(crate::primitives::XPubId),
    #[error("Could not parse incoming psbt: {0}")]
    CouldNotParseIncomingPsbt(bitcoin::psbt::PsbtParseError),
    #[error("Hex decode error: {0}")]
    HexDecodeError(#[from] hex::FromHexError),
    #[error("Could not decrypt the encrypted key: {0}")]
    CouldNotDecryptKey(chacha20poly1305::Error),
    #[error("AddressError - Could not parse the address: {0}")]
    CouldNotParseAddress(#[from] bitcoin::AddressError),
}

impl From<chacha20poly1305::Error> for ApplicationError {
    fn from(value: chacha20poly1305::Error) -> Self {
        ApplicationError::CouldNotDecryptKey(value)
    }
}
