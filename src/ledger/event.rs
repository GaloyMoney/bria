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
    UtxoDetected(IncomingUtxoMeta),
    UtxoSettled(ConfirmedUtxoMeta),
    SpendDetected(ExternalSpendMeta),
    SpendSettled(ConfirmSpendMeta),
    PayoutQueued(QueuedPayoutMeta),
    BatchCreated(CreateBatchMeta),
    BatchSubmitted(SubmitBatchMeta),
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
                    INCOMING_UTXO_ID => JournalEventMetadata::UtxoDetected(
                        tx.metadata::<IncomingUtxoMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    CONFIRMED_UTXO_ID | CONFIRM_SPENT_UTXO_ID => JournalEventMetadata::UtxoSettled(
                        tx.metadata::<ConfirmedUtxoMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    EXTERNAL_SPEND_ID => JournalEventMetadata::SpendDetected(
                        tx.metadata::<ExternalSpendMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    CONFIRM_SPEND_ID => JournalEventMetadata::SpendSettled(
                        tx.metadata::<ConfirmSpendMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    QUEUED_PAYOUD_ID => JournalEventMetadata::PayoutQueued(
                        tx.metadata::<QueuedPayoutMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    CREATE_BATCH_ID => JournalEventMetadata::BatchCreated(
                        tx.metadata::<CreateBatchMeta>()?
                            .ok_or(BriaError::MissingTxMetadata)?,
                    ),
                    SUBMIT_BATCH_ID => JournalEventMetadata::BatchSubmitted(
                        tx.metadata::<SubmitBatchMeta>()?
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
