use serde::{Deserialize, Serialize};
use sqlx_ledger::event::*;

use super::{constants::*, templates::*};

use crate::error::*;

#[derive(Serialize, Deserialize)]
pub struct JournalEvent {
    pub ledger_event_id: u64,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub r#type: EventType,
    pub metadata: EventMetadata,
    #[serde(skip, default)]
    pub notification_span: Option<tracing::Span>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    IncomingUtxo,
    ConfirmedUtxo,
    ExternalSpend,
    ConfirmSpend,
    QueuedPayout,
    CreateBatch,
    SubmitBatch,
    Unknown,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventMetadata {
    IncomingUtxo(IncomingUtxoMeta),
    ConfirmedUtxo(ConfirmedUtxoMeta),
    ExternalSpend(ExternalSpendMeta),
    ConfirmSpend(ConfirmSpendMeta),
    QueuedPayout(QueuedPayoutMeta),
    CreateBatch(CreateBatchMeta),
    SubmitBatch(SubmitBatchMeta),
    UnknownTransaction(Option<serde_json::Value>),
}

impl EventMetadata {
    pub fn r#type(&self) -> EventType {
        match self {
            EventMetadata::IncomingUtxo(_) => EventType::IncomingUtxo,
            EventMetadata::ConfirmedUtxo(_) => EventType::ConfirmedUtxo,
            EventMetadata::ExternalSpend(_) => EventType::ExternalSpend,
            EventMetadata::ConfirmSpend(_) => EventType::ConfirmSpend,
            EventMetadata::QueuedPayout(_) => EventType::QueuedPayout,
            EventMetadata::CreateBatch(_) => EventType::CreateBatch,
            EventMetadata::SubmitBatch(_) => EventType::SubmitBatch,
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
                    INCOMING_UTXO_ID => EventMetadata::IncomingUtxo(
                        tx.metadata::<IncomingUtxoMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    CONFIRMED_UTXO_ID | CONFIRM_SPENT_UTXO_ID => EventMetadata::ConfirmedUtxo(
                        tx.metadata::<ConfirmedUtxoMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    EXTERNAL_SPEND_ID => EventMetadata::ExternalSpend(
                        tx.metadata::<ExternalSpendMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    CONFIRM_SPEND_ID => EventMetadata::ConfirmSpend(
                        tx.metadata::<ConfirmSpendMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    QUEUED_PAYOUD_ID => EventMetadata::QueuedPayout(
                        tx.metadata::<QueuedPayoutMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    CREATE_BATCH_ID => EventMetadata::CreateBatch(
                        tx.metadata::<CreateBatchMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    SUBMIT_BATCH_ID => EventMetadata::SubmitBatch(
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
            notification_span: Some(event.span),
        }))
    }
}
