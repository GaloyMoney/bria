use thiserror::Error;

#[derive(Error, Debug)]
pub enum JobSvcError {
    #[error("JobSvcError - ConfigBuild: {0}")]
    ConfigBuild(String),
    #[error("JobSvcError - JobCrateError: {0}")]
    JobCrateError(#[from] job_crate::error::JobError),
}
