mod augmentation;
pub mod error;
mod event;
mod listener;
mod repo;

use opentelemetry::trace::TraceContextExt;
use sqlx::{postgres::PgListener, Pool, Postgres};
use tokio::sync::{broadcast, RwLock};
use tracing::instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use std::{collections::HashMap, sync::Arc};

use crate::{ledger::*, primitives::*};

pub use augmentation::*;
use error::OutboxError;
pub use event::*;
pub use listener::*;
use repo::*;

type SequenceElems = (EventSequence, Option<SqlxLedgerEventId>);
type SequenceMap = HashMap<AccountId, Arc<RwLock<SequenceElems>>>;

const DEFAULT_BUFFER_SIZE: usize = 100;

#[derive(Clone)]
pub struct Outbox {
    _pool: Pool<Postgres>,
    repo: OutboxRepo,
    augmenter: Augmenter,
    sequences: Arc<RwLock<SequenceMap>>,
    event_sender: broadcast::Sender<OutboxEvent<WithoutAugmentation>>,
    event_receiver: Arc<broadcast::Receiver<OutboxEvent<WithoutAugmentation>>>,
    buffer_size: usize,
}

impl Outbox {
    pub async fn init(pool: &Pool<Postgres>, augmenter: Augmenter) -> Result<Self, OutboxError> {
        let buffer_size = DEFAULT_BUFFER_SIZE;
        let (sender, recv) = broadcast::channel(buffer_size);
        let sequences = Arc::new(RwLock::new(HashMap::new()));
        let repo = OutboxRepo::new(pool);
        Self::spawn_pg_listener(pool, sender.clone(), repo.clone(), Arc::clone(&sequences)).await?;

        let ret = Self {
            _pool: pool.clone(),
            augmenter,
            repo,
            sequences,
            event_sender: sender,
            event_receiver: Arc::new(recv),
            buffer_size,
        };
        Ok(ret)
    }

    #[instrument(
        "outbox.handle_journal_event",
        skip(self, linked_span),
        fields(next_sequence, ledger_event, error, error.level, error.message),
        err
    )]
    pub async fn handle_journal_event(
        &self,
        mut ledger_event: JournalEvent,
        linked_span: tracing::Span,
    ) -> Result<(), OutboxError> {
        let current_span = tracing::Span::current();
        current_span.add_link(linked_span.context().span().span_context().clone());
        if let Some(context) = ledger_event.notification_otel_context.take() {
            current_span.set_parent(context);
        }

        current_span.record(
            "ledger_event",
            tracing::field::display(
                serde_json::to_string(&ledger_event).expect("Couldn't serialize JournalEvent"),
            ),
        );
        let payloads = Vec::<OutboxEventPayload>::from(ledger_event.metadata);
        let sequences = self.sequences_for(ledger_event.account_id).await?;
        let mut write_sequences = sequences.write().await;
        let mut sequence = write_sequences.0;
        current_span.record("next_sequence", tracing::field::display(sequence));
        let events: Vec<OutboxEvent<_>> = payloads
            .into_iter()
            .map(|payload| {
                sequence = sequence.next();
                OutboxEvent::builder()
                    .account_id(ledger_event.account_id)
                    .sequence(sequence)
                    .payload(payload)
                    .ledger_event_id(ledger_event.ledger_event_id)
                    .ledger_tx_id(ledger_event.ledger_tx_id)
                    .recorded_at(ledger_event.recorded_at)
                    .build()
                    .expect("Could not build OutboxEvent")
            })
            .collect();

        if let Err(res) = self.repo.persist_events(&events).await {
            let mut write_seqs = self.sequences.write().await;
            write_seqs.remove(&ledger_event.account_id);
            crate::tracing::insert_error_fields(tracing::Level::WARN, &res);
            return Err(res);
        }
        for event in events {
            self.event_sender
                .send(event)
                .map_err(|_| OutboxError::SendEventError)?;
        }

        *write_sequences = (sequence, Some(ledger_event.ledger_event_id));

        Ok(())
    }

    pub async fn register_listener(
        &self,
        account_id: AccountId,
        start_after: Option<EventSequence>,
        augment: bool,
    ) -> Result<OutboxListener, OutboxError> {
        let sub = self.event_receiver.resubscribe();
        let latest_known = self.sequences_for(account_id).await?.read().await.0;
        let start = start_after.unwrap_or(latest_known);
        Ok(OutboxListener::new(
            self.repo.clone(),
            augment.then(|| self.augmenter.clone()),
            sub,
            account_id,
            start,
            latest_known,
            self.buffer_size,
        ))
    }

    #[instrument("outbox.last_ledger_event_id", skip(self), ret, err)]
    pub async fn last_ledger_event_id(
        &self,
        account_id: AccountId,
    ) -> Result<Option<SqlxLedgerEventId>, OutboxError> {
        let sequences = self.sequences_for(account_id).await?;
        let read_seq = sequences.read().await;
        Ok(read_seq.1)
    }

    async fn spawn_pg_listener(
        pool: &Pool<Postgres>,
        sender: broadcast::Sender<OutboxEvent<WithoutAugmentation>>,
        repo: OutboxRepo,
        sequences: Arc<RwLock<SequenceMap>>,
    ) -> Result<(), OutboxError> {
        let mut listener = PgListener::connect_with(pool).await?;
        listener.listen("bria_outbox_events").await?;
        tokio::spawn(async move {
            loop {
                if let Ok(notification) = listener.recv().await {
                    if let Ok(event) = serde_json::from_str::<OutboxEvent<WithoutAugmentation>>(
                        notification.payload(),
                    ) {
                        let (account_id, sequence, ledger_id) =
                            (event.account_id, event.sequence, event.ledger_event_id);
                        if sender.send(event).is_err() {
                            break;
                        }
                        if let Ok(sequence_ref) =
                            Self::sequences_for_inner(&repo, &sequences, account_id).await
                        {
                            let mut write_seq_ref = sequence_ref.write().await;
                            if write_seq_ref.0 < sequence {
                                write_seq_ref.0 = sequence;
                                if let Some(ledger_id) = ledger_id {
                                    write_seq_ref.1 = Some(ledger_id);
                                }
                            }
                        }
                    }
                }
            }
        });
        Ok(())
    }

    async fn sequences_for(
        &self,
        account_id: AccountId,
    ) -> Result<Arc<RwLock<SequenceElems>>, OutboxError> {
        Self::sequences_for_inner(&self.repo, &self.sequences, account_id).await
    }

    async fn sequences_for_inner(
        repo: &OutboxRepo,
        sequences: &Arc<RwLock<SequenceMap>>,
        account_id: AccountId,
    ) -> Result<Arc<RwLock<SequenceElems>>, OutboxError> {
        {
            let read_map = sequences.read().await;
            if let Some(elems) = read_map.get(&account_id) {
                return Ok(Arc::clone(elems));
            }
        }
        let mut write_map = sequences.write().await;
        *write_map = repo.load_latest_sequences().await?;
        let res = write_map
            .entry(account_id)
            .or_insert_with(|| Arc::new(RwLock::new((EventSequence::BEGIN, None))));
        Ok(Arc::clone(res))
    }
}
