use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use job_crate::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DummyJobConfig;

impl JobConfig for DummyJobConfig {
    type Initializer = DummyJobInit;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DummyJobInit;
impl JobInitializer for DummyJobInit {
    fn job_type() -> JobType {
        JobType::new("dummy")
    }

    fn init(&self, _job: &Job) -> Result<Box<dyn JobRunner>, Box<dyn std::error::Error>> {
        Ok(Box::new(DummyJobRunner))
    }
}

struct DummyJobRunner;

#[async_trait]
impl JobRunner for DummyJobRunner {
    async fn run(&self, _current_job: CurrentJob) -> Result<JobCompletion, Box<dyn std::error::Error>> {
        tracing::info!("Dummy job running!");
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        tracing::info!("Dummy job completed successfully");
        Ok(JobCompletion::Complete)
    }
}
