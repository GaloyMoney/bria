use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::error::JobError;
use crate::{ledger::*, outbox::*, primitives::*};

use std::collections::HashMap;

#[instrument("job.import_mempool")]
pub async fn execute() -> Result<(), JobError> {
    Ok(())
}
