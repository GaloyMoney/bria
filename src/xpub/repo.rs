use sqlx::{Pool, Postgres};
use uuid::Uuid;

use crate::{error::*, primitives::*};

pub struct XPubs {
    pool: Pool<Postgres>,
}

impl XPubs {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn persist(
        &self,
        account_id: AccountId,
        name: String,
        xpub: String,
    ) -> Result<XPubId, BriaError> {
        let record = sqlx::query!(
            r#"INSERT INTO xpubs (account_id, name, xpub)
            VALUES ((SELECT id FROM accounts WHERE id = $1), $2, $3) RETURNING (id)"#,
            Uuid::from(account_id),
            name,
            xpub
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(XPubId::from(record.id))
    }
}
