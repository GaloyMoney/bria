use sqlx::{Pool, Postgres};
use tracing::instrument;

use crate::{error::*, ledger::*};
use opentelemetry::trace::TraceContextExt;
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[derive(Clone)]
pub struct Outbox {
    _pool: Pool<Postgres>,
}

impl Outbox {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self {
            _pool: pool.clone(),
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
                let _address = income.address;
            }
            _ => (),
        }
        Ok(())
    }
}
