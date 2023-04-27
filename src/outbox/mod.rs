mod event;
mod repo;

use sqlx::{Pool, Postgres};
use tracing::instrument;

use crate::{address::*, error::*, ledger::*};
use opentelemetry::trace::TraceContextExt;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use repo::*;

#[derive(Clone)]
pub struct Outbox {
    _pool: Pool<Postgres>,
    addresses: Addresses,
    _repo: OutboxRepo,
}

impl Outbox {
    pub fn new(pool: &Pool<Postgres>, addresses: Addresses) -> Self {
        Self {
            _repo: OutboxRepo::new(pool),
            _pool: pool.clone(),
            addresses,
        }
    }

    #[instrument("outbox.handle_journal_event", skip(self, linked_span))]
    pub async fn handle_journal_event(
        &self,
        mut event: JournalEvent,
        linked_span: tracing::Span,
    ) -> Result<(), BriaError> {
        let current_span = tracing::Span::current();
        current_span.add_link(linked_span.context().span().span_context().clone());
        if let Some(context) = event.notification_otel_context.take() {
            current_span.set_parent(context);
        }
        match event.metadata {
            EventMetadata::UtxoDetected(income) => {
                let _address = self
                    .addresses
                    .find_by_address(income.account_id, income.address)
                    .await?;
            }
            EventMetadata::UtxoSettled(income) => {
                let _address = self
                    .addresses
                    .find_by_address(income.account_id, income.address)
                    .await?;
            }
            _ => (),
        }
        Ok(())
    }
}
