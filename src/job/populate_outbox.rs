use futures::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx_ledger::JournalId;
use tracing::instrument;

use std::collections::HashMap;

use crate::{error::*, ledger::*, outbox::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulateOutboxData {
    pub(super) account_id: AccountId,
    pub(super) journal_id: JournalId,
    #[serde(flatten)]
    pub(super) tracing_data: HashMap<String, String>,
}

#[instrument("job.handle_outbox", skip(outbox, ledger))]
pub async fn execute(
    data: PopulateOutboxData,
    outbox: Outbox,
    ledger: Ledger,
) -> Result<PopulateOutboxData, BriaError> {
    let mut stream = ledger.journal_events(data.journal_id, 0).await?;
    while let Some(event) = stream.next().await {
        outbox
            .handle_journal_event(event?, tracing::Span::current())
            .await?;
    }
    Ok(data)
}
