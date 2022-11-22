use sqlx::PgPool;
use uuid::Uuid;

use super::convert::BdkKeychainKind;
use crate::primitives::*;

pub struct Indexes {
    pool: PgPool,
    keychain_id: KeychainId,
}

impl Indexes {
    pub fn new(keychain_id: KeychainId, pool: PgPool) -> Self {
        Self { keychain_id, pool }
    }

    pub async fn increment(&self, keychain: impl Into<BdkKeychainKind>) -> Result<u32, bdk::Error> {
        let kind = keychain.into();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        let rows = sqlx::query!(
            r#"SELECT index FROM bdk_indexes
          WHERE keychain_id = $1 AND keychain_kind = $2 ORDER BY index DESC LIMIT 1"#,
            Uuid::from(self.keychain_id),
            kind as BdkKeychainKind
        )
        .fetch_all(&mut tx)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        let new_idx = rows.get(0).map(|row| row.index + 1).unwrap_or(0);
        sqlx::query!(
            r#"INSERT INTO bdk_indexes (keychain_id, keychain_kind, index)
                VALUES ($1, $2, $3)"#,
            Uuid::from(self.keychain_id),
            kind as BdkKeychainKind,
            new_idx
        )
        .execute(&mut tx)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(new_idx as u32)
    }
}
