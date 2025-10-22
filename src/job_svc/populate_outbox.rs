use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};

use job_crate::{
    CurrentJob, Job, JobCompletion, JobConfig, JobInitializer, JobRunner, JobType, RetrySettings,
};

use crate::{
    ledger::Ledger,
    outbox::Outbox,
    primitives::{AccountId, LedgerJournalId},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulateOutboxJobConfig {
    pub account_id: AccountId,
    pub journal_id: LedgerJournalId,
}

impl JobConfig for PopulateOutboxJobConfig {
    type Initializer = PopulateOutboxJobInit;
}

pub struct PopulateOutboxJobInit {
    outbox: Outbox,
    ledger: Ledger,
}

impl PopulateOutboxJobInit {
    pub fn new(outbox: Outbox, ledger: Ledger) -> Self {
        Self { outbox, ledger }
    }
}

impl JobInitializer for PopulateOutboxJobInit {
    fn job_type() -> JobType
    where
        Self: Sized,
    {
        JobType::new("populate_outbox")
    }

    fn init(&self, job: &Job) -> Result<Box<dyn JobRunner>, Box<dyn std::error::Error>> {
        let config: PopulateOutboxJobConfig = job.config()?;
        Ok(Box::new(PopulateOutboxJobRunner {
            config,
            outbox: self.outbox.clone(),
            ledger: self.ledger.clone(),
        }))
    }

    fn retry_on_error_settings() -> RetrySettings
    where
        Self: Sized,
    {
        RetrySettings::repeat_indefinitely()
    }
}

pub struct PopulateOutboxJobRunner {
    config: PopulateOutboxJobConfig,
    outbox: Outbox,
    ledger: Ledger,
}

#[async_trait]
impl JobRunner for PopulateOutboxJobRunner {
    async fn run(
        &self,
        _current_job: CurrentJob,
    ) -> Result<JobCompletion, Box<dyn std::error::Error>> {
        let mut stream = self
            .ledger
            .journal_events(
                self.config.journal_id,
                self.outbox
                    .last_ledger_event_id(self.config.account_id)
                    .await?,
            )
            .await?;

        while let Some(event) = stream.next().await {
            self.outbox
                .handle_journal_event(event?, tracing::Span::current())
                .await?;
        }

        Ok(JobCompletion::Complete)
    }
}
