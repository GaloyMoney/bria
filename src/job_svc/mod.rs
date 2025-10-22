pub mod error;
mod populate_outbox;

use job_crate::{JobId, JobSvcConfig, Jobs};
use tracing::instrument;

use crate::job_svc::populate_outbox::{PopulateOutboxJobConfig, PopulateOutboxJobInit};
use crate::{
    ledger::Ledger,
    outbox::Outbox,
    primitives::{AccountId, LedgerJournalId},
};

pub use error::JobSvcError;

#[derive(Clone)]
pub struct JobSvc {
    jobs: Jobs,
}

impl JobSvc {
    pub async fn init(
        pool: sqlx::PgPool,
        outbox: Outbox,
        ledger: Ledger,
    ) -> Result<Self, JobSvcError> {
        let job_svc_config = JobSvcConfig::builder()
            .pool(pool)
            .build()
            .expect("Couldn't build JobSvcConfig");

        let mut jobs = Jobs::init(job_svc_config).await?;
        jobs.add_initializer(PopulateOutboxJobInit::new(outbox, ledger));
        jobs.start_poll().await?;

        Ok(Self { jobs })
    }

    pub fn jobs(&self) -> &Jobs {
        &self.jobs
    }

    #[instrument(name = "job_svc.spawn_outbox_handler_in_op", skip_all)]
    pub async fn spawn_outbox_handler_in_op(
        &self,
        op: &mut impl es_entity::AtomicOperation,
        account_id: AccountId,
        journal_id: LedgerJournalId,
    ) -> Result<(), JobSvcError> {
        let config = PopulateOutboxJobConfig {
            account_id,
            journal_id,
        };

        let job_id = JobId::from(uuid::Uuid::from(config.journal_id));

        self.jobs.create_and_spawn_in_op(op, job_id, config).await?;
        Ok(())
    }
}
