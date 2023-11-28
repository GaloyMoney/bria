use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::error::JobError;
use crate::{ledger::*, outbox::*, primitives::*};

use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulateOutboxData {
    pub(super) account_id: AccountId,
    pub(super) journal_id: LedgerJournalId,
    #[serde(flatten)]
    pub(super) tracing_data: HashMap<String, String>,
}

#[instrument("job.populate_outbox", skip(outbox, ledger))]
pub async fn execute(
    data: PopulateOutboxData,
    outbox: Outbox,
    ledger: Ledger,
) -> Result<PopulateOutboxData, JobError> {
    let mut stream = ledger
        .journal_events(
            data.journal_id,
            outbox.last_ledger_event_id(data.account_id).await?,
        )
        .await?;
    while let Some(event) = stream.next().await {
        outbox
            .handle_journal_event(event?, tracing::Span::current())
            .await?;
    }
    Ok(data)
}
