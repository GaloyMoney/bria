mod populate_outbox;

use job_crate::Jobs;

use crate::job_svc::populate_outbox::PopulateOutboxJobInit;
use crate::{ledger::Ledger, outbox::Outbox};

pub use populate_outbox::spawn_outbox_handler;

pub struct JobSvc;

impl JobSvc {
    pub fn init(jobs: &Jobs, outbox: Outbox, ledger: Ledger) {
        jobs.add_initializer(PopulateOutboxJobInit::new(outbox, ledger));
    }
}
