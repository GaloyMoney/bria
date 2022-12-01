use crate::error::*;
use sqlx::PgPool;
use sqlx_ledger::SqlxLedger;

pub struct Ledger {
    inner: SqlxLedger,
}

impl Ledger {
    pub async fn init(pool: PgPool) -> Result<Self, BriaError> {
        let inner = SqlxLedger::new(&pool);
        Ok(Self { inner })
    }
}
