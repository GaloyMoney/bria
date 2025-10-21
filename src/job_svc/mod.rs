pub mod error;
mod populate_outbox;

use job_crate::{JobId, JobSvcConfig, Jobs};
use tracing::instrument;

use crate::job_svc::populate_outbox::PopulateOutboxJobInit;
use crate::{account::Account, ledger::Ledger, outbox::Outbox};

pub use error::JobSvcError;
pub use populate_outbox::PopulateOutboxJobConfig;

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
            .map_err(|e| JobSvcError::ConfigBuild(e.to_string()))?;

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
        account: Account,
    ) -> Result<(), JobSvcError> {
        let config = PopulateOutboxJobConfig {
            account_id: account.id,
            journal_id: account.journal_id(),
            tracing_data: crate::tracing::extract_tracing_data(),
        };

        let job_id = JobId::from(uuid::Uuid::from(config.journal_id));

        self.jobs.create_and_spawn_in_op(op, job_id, config).await?;
        Ok(())
    }
}
