use bdk::{bitcoin::Txid, TransactionDetails};
use sqlx::PgPool;
use uuid::Uuid;

use crate::primitives::*;

pub struct Transactions {
    keychain_id: KeychainId,
    pool: PgPool,
}

impl Transactions {
    pub fn new(keychain_id: KeychainId, pool: PgPool) -> Self {
        Self { keychain_id, pool }
    }

    pub async fn persist(&self, tx: &TransactionDetails) -> Result<(), bdk::Error> {
        sqlx::query!(
            r#"
        INSERT INTO bdk_transactions (keychain_id, tx_id, details_json)
        VALUES ($1, $2, $3) ON CONFLICT (keychain_id, tx_id)
        DO UPDATE SET details_json = EXCLUDED.details_json"#,
            Uuid::from(self.keychain_id),
            tx.txid.to_string(),
            serde_json::to_value(&tx)?,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(())
    }

    pub async fn find_by_id(&self, tx_id: &Txid) -> Result<Option<TransactionDetails>, bdk::Error> {
        let tx = sqlx::query!(
            r#"
        SELECT details_json FROM bdk_transactions WHERE keychain_id = $1 AND tx_id = $2"#,
            Uuid::from(self.keychain_id),
            tx_id.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(tx.map(|tx| serde_json::from_value(tx.details_json).unwrap()))
    }

    pub async fn list(&self) -> Result<Vec<TransactionDetails>, bdk::Error> {
        let txs = sqlx::query!(
            r#"
        SELECT details_json FROM bdk_transactions WHERE keychain_id = $1"#,
            Uuid::from(self.keychain_id),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(txs
            .into_iter()
            .map(|tx| serde_json::from_value(tx.details_json).unwrap())
            .collect())
    }
}
