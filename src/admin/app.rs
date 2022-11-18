use super::error::*;

pub struct AdminApp {
    pool: sqlx::PgPool,
}

impl AdminApp {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

impl AdminApp {
    pub async fn bootstrap(&self) -> Result<String, AdminApiError> {
        Ok("Hello, world!".to_string())
    }
}
