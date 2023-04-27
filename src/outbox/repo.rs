use sqlx::{Pool, Postgres};

#[derive(Clone)]
pub(super) struct OutboxRepo {
    _pool: Pool<Postgres>,
}

impl OutboxRepo {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self {
            _pool: pool.clone(),
        }
    }
}
