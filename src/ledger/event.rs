use serde::{Deserialize, Serialize};
use sqlx_ledger::event::*;

use super::{constants::*, templates::*};
use crate::error::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct JournalEvent {
    pub ledger_event_id: u64,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub r#type: EventType,
    pub metadata: EventMetadata,
    #[serde(skip, default)]
    pub notification_otel_context: Option<opentelemetry::Context>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    UtxoDetected,
    UtxoSettled,
    SpendDetected,
    SpendSettled,
    PayoutQueued,
    BatchCreated,
    BatchSubmitted,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventMetadata {
    UtxoDetected(IncomingUtxoMeta),
    UtxoSettled(ConfirmedUtxoMeta),
    SpendDetected(ExternalSpendMeta),
    SpendSettled(ConfirmSpendMeta),
    PayoutQueued(QueuedPayoutMeta),
    BatchCreated(CreateBatchMeta),
    BatchSubmitted(SubmitBatchMeta),
    UnknownTransaction(Option<serde_json::Value>),
}

impl EventMetadata {
    pub fn r#type(&self) -> EventType {
        match self {
            EventMetadata::UtxoDetected(_) => EventType::UtxoDetected,
            EventMetadata::UtxoSettled(_) => EventType::UtxoSettled,
            EventMetadata::SpendDetected(_) => EventType::SpendDetected,
            EventMetadata::SpendSettled(_) => EventType::SpendSettled,
            EventMetadata::PayoutQueued(_) => EventType::PayoutQueued,
            EventMetadata::BatchCreated(_) => EventType::BatchCreated,
            EventMetadata::BatchSubmitted(_) => EventType::BatchSubmitted,
            EventMetadata::UnknownTransaction(_) => EventType::Unknown,
        }
    }
}

pub(super) enum MaybeIgnored {
    Ignored,
    Event(JournalEvent),
}

impl TryFrom<SqlxLedgerEvent> for MaybeIgnored {
    type Error = BriaError;

    fn try_from(event: SqlxLedgerEvent) -> Result<Self, Self::Error> {
        let metadata = match event.data {
            SqlxLedgerEventData::TransactionCreated(tx) => {
                match uuid::Uuid::from(tx.tx_template_id) {
                    INCOMING_UTXO_ID => EventMetadata::UtxoDetected(
                        tx.metadata::<IncomingUtxoMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    CONFIRMED_UTXO_ID | CONFIRM_SPENT_UTXO_ID => EventMetadata::UtxoSettled(
                        tx.metadata::<ConfirmedUtxoMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    EXTERNAL_SPEND_ID => EventMetadata::SpendDetected(
                        tx.metadata::<ExternalSpendMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    CONFIRM_SPEND_ID => EventMetadata::SpendSettled(
                        tx.metadata::<ConfirmSpendMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    QUEUED_PAYOUD_ID => EventMetadata::PayoutQueued(
                        tx.metadata::<QueuedPayoutMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    CREATE_BATCH_ID => EventMetadata::BatchCreated(
                        tx.metadata::<CreateBatchMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    SUBMIT_BATCH_ID => EventMetadata::BatchSubmitted(
                        tx.metadata::<SubmitBatchMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    _ => EventMetadata::UnknownTransaction(tx.metadata_json),
                }
            }
            _ => {
                return Ok(MaybeIgnored::Ignored);
            }
        };
        Ok(MaybeIgnored::Event(JournalEvent {
            ledger_event_id: event.id,
            recorded_at: event.recorded_at,
            r#type: metadata.r#type(),
            metadata,
            notification_otel_context: Some(event.otel_context),
        }))
    }
}
