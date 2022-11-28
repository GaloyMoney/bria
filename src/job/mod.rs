use sqlxmq::{JobRegistry, OwnedHandle};

use crate::error::*;

pub async fn start_job_runner(pool: &sqlx::PgPool) -> Result<OwnedHandle, BriaError> {
    let registry = JobRegistry::new(&[]);

    Ok(registry.runner(pool).run().await?)
}
