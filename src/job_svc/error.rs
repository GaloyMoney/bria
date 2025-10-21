use thiserror::Error;

#[derive(Error, Debug)]
pub enum JobSvcError {
    #[error("JobSvcError - JobCrateError: {0}")]
    JobCrateError(#[from] job_crate::error::JobError),
}
