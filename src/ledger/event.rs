use sqlx_ledger::event::*;

use super::{constants::*, templates::*};
use crate::{error::*, primitives::*};

#[derive(Clone, Debug)]
pub struct JournalEvent {
    pub journal_id: LedgerJournalId,
    pub account_id: AccountId,
    pub ledger_tx_id: LedgerTransactionId,
    pub ledger_event_id: SqlxLedgerEventId,
    pub recorded_at: chrono::DateTime<chrono::Utc>,
    pub metadata: JournalEventMetadata,
    pub notification_otel_context: Option<opentelemetry::Context>,
}

#[derive(Clone, Debug)]
pub enum JournalEventMetadata {
    UtxoDetected(UtxoDetectedMeta),
    UtxoSettled(UtxoSettledMeta),
    SpendDetected(SpendDetectedMeta),
    SpendSettled(SpendSettledMeta),
    PayoutQueued(PayoutQueuedMeta),
    BatchCreated(BatchCreatedMeta),
    BatchSubmitted(BatchSubmittedMeta),
    UnknownTransaction(Option<serde_json::Value>),
}

pub(super) enum MaybeIgnored {
    Ignored,
    Event(JournalEvent),
}

impl TryFrom<SqlxLedgerEvent> for MaybeIgnored {
    type Error = BriaError;

    fn try_from(event: SqlxLedgerEvent) -> Result<Self, Self::Error> {
        let journal_id = event.journal_id();
        let (tx_id, metadata) = match event.data {
            SqlxLedgerEventData::TransactionCreated(tx) => (
                tx.id,
                match uuid::Uuid::from(tx.tx_template_id) {
                    UTXO_DETECTED_ID => JournalEventMetadata::UtxoDetected(
                        tx.metadata::<UtxoDetectedMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    UTXO_SETTLED_ID | SPENT_UTXO_SETTLED_ID => JournalEventMetadata::UtxoSettled(
                        tx.metadata::<UtxoSettledMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    SPEND_DETECTED_ID => JournalEventMetadata::SpendDetected(
                        tx.metadata::<SpendDetectedMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    SPEND_SETTLED_ID => JournalEventMetadata::SpendSettled(
                        tx.metadata::<SpendSettledMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    PAYOUT_QUEUED_ID => JournalEventMetadata::PayoutQueued(
                        tx.metadata::<PayoutQueuedMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    BATCH_CREATED_ID => JournalEventMetadata::BatchCreated(
                        tx.metadata::<BatchCreatedMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    BATCH_SUBMITTED_ID => JournalEventMetadata::BatchSubmitted(
                        tx.metadata::<BatchSubmittedMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    _ => JournalEventMetadata::UnknownTransaction(tx.metadata_json),
                },
            ),
            _ => {
                return Ok(MaybeIgnored::Ignored);
            }
        };
        Ok(MaybeIgnored::Event(JournalEvent {
            journal_id,
            account_id: AccountId::from(journal_id),
            ledger_tx_id: tx_id,
            ledger_event_id: event.id,
            recorded_at: event.recorded_at,
            metadata,
            notification_otel_context: Some(event.otel_context),
        }))
    }
}
