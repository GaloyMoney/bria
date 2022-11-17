use sqlx::{Pool, Postgres};

pub struct AdminApiKeys {
    pool: Pool<Postgres>,
}

impl AdminApiKeys {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub fn create(&self) -> Result<(), ()> {
        Ok(())
    }
}
