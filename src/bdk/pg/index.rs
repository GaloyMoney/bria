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
        let result = sqlx::query!(
            r#"
              INSERT INTO bdk_indexes (keychain_id, keychain_kind)
              VALUES ($1, $2)
              ON CONFLICT (keychain_id, keychain_kind)
              DO UPDATE SET index = bdk_indexes.index + 1, modified_at = NOW()
              WHERE bdk_indexes.keychain_id = $1 AND bdk_indexes.keychain_kind = $2
              RETURNING index;
              "#,
            Uuid::from(self.keychain_id),
            kind as BdkKeychainKind
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;

        let new_idx = result.index;
        Ok(new_idx as u32)
    }

    pub async fn persist_last_index(
        &self,
        keychain: impl Into<BdkKeychainKind>,
        idx: u32,
    ) -> Result<(), bdk::Error> {
        sqlx::query!(
            r#"UPDATE bdk_indexes
                 SET index = $1, modified_at = NOW()
                 WHERE index < $1 AND keychain_id = $2 AND keychain_kind = $3"#,
            idx as i32,
            self.keychain_id as KeychainId,
            keychain.into() as BdkKeychainKind,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(())
    }

    pub async fn get_latest(
        &self,
        keychain: impl Into<BdkKeychainKind>,
    ) -> Result<Option<u32>, bdk::Error> {
        let kind = keychain.into();
        let rows = sqlx::query!(
            r#"SELECT index FROM bdk_indexes WHERE keychain_id = $1 AND keychain_kind = $2"#,
            Uuid::from(self.keychain_id),
            kind as BdkKeychainKind
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(rows.get(0).map(|row| row.index as u32))
    }
}
