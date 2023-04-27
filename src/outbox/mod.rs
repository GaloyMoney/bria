mod event;
mod repo;

use opentelemetry::trace::TraceContextExt;
use sqlx::{Pool, Postgres};
use tokio::sync::RwLock;
use tracing::instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use std::{collections::HashMap, sync::Arc};

use crate::{address::*, error::*, ledger::*, primitives::*};

pub use event::*;
use repo::*;

type SequenceElems = (EventSequence, Option<SqlxLedgerEventId>);
type SequenceMap = HashMap<AccountId, Arc<RwLock<SequenceElems>>>;

#[derive(Clone)]
pub struct Outbox {
    _pool: Pool<Postgres>,
    _addresses: Addresses,
    repo: OutboxRepo,
    sequences: Arc<RwLock<SequenceMap>>,
}

impl Outbox {
    pub fn new(pool: &Pool<Postgres>, addresses: Addresses) -> Self {
        Self {
            _pool: pool.clone(),
            repo: OutboxRepo::new(pool),
            _addresses: addresses,
            sequences: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[instrument("outbox.handle_journal_event", skip(self, linked_span))]
    pub async fn handle_journal_event(
        &self,
        mut ledger_event: JournalEvent,
        linked_span: tracing::Span,
    ) -> Result<(), BriaError> {
        let current_span = tracing::Span::current();
        current_span.add_link(linked_span.context().span().span_context().clone());
        if let Some(context) = ledger_event.notification_otel_context.take() {
            current_span.set_parent(context);
        }

        if let Ok(payload) = OutboxEventPayload::try_from(ledger_event.metadata) {
            let sequences = self.sequences_for(ledger_event.account_id).await?;
            let mut write_sequences = sequences.write().await;
            let next_sequence = write_sequences.0.next();
            let event = OutboxEvent::builder()
                .account_id(ledger_event.account_id)
                .sequence(next_sequence)
                .payload(payload)
                .ledger_event_id(ledger_event.ledger_event_id)
                .ledger_tx_id(ledger_event.ledger_tx_id)
                .recorded_at(ledger_event.recorded_at)
                .build()
                .expect("Could not build OutboxEvent");

            if let Err(res) = self.repo.persist_event(event).await {
                let mut write_seqs = self.sequences.write().await;
                write_seqs.remove(&ledger_event.account_id);
                return Err(res);
            }

            *write_sequences = (next_sequence, Some(ledger_event.ledger_event_id));
        }

        Ok(())
    }

    #[instrument("outbox.last_ledger_event_id", skip(self), ret, err)]
    pub async fn last_ledger_event_id(
        &self,
        account_id: AccountId,
    ) -> Result<Option<SqlxLedgerEventId>, BriaError> {
        let sequences = self.sequences_for(account_id).await?;
        let read_seq = sequences.read().await;
        Ok(read_seq.1)
    }

    async fn sequences_for(
        &self,
        account_id: AccountId,
    ) -> Result<Arc<RwLock<SequenceElems>>, BriaError> {
        {
            let read_map = self.sequences.read().await;
            if let Some(elems) = read_map.get(&account_id) {
                return Ok(Arc::clone(elems));
            }
        }
        let mut write_map = self.sequences.write().await;
        *write_map = self.repo.load_latest_sequences().await?;
        let res = write_map
            .entry(account_id)
            .or_insert_with(|| Arc::new(RwLock::new((EventSequence::BEGIN, None))));
        Ok(Arc::clone(res))
    }
}
