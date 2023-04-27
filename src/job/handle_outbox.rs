use serde::{Deserialize, Serialize};
use sqlx_ledger::JournalId;
use tracing::instrument;

use std::collections::HashMap;
use std::time::Duration;

use crate::{error::*, ledger::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleOutboxData {
    pub(super) account_id: AccountId,
    pub(super) journal_id: JournalId,
    #[serde(flatten)]
    pub(super) tracing_data: HashMap<String, String>,
}

#[instrument("job.handle_outbox")]
pub async fn execute(
    data: HandleOutboxData,
    ledger: Ledger,
) -> Result<HandleOutboxData, BriaError> {
    let start_time = tokio::time::Instant::now();
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        if start_time.elapsed() >= Duration::from_secs(120) {
            break;
        }
    }
    Ok(data)
}
