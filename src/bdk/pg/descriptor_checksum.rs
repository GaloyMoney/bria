use bdk::KeychainKind;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::convert::KeychainKindPg;
use crate::primitives::*;

pub struct DescriptorChecksums {
    keychain_id: KeychainId,
}

impl DescriptorChecksums {
    pub fn new(keychain_id: KeychainId) -> Self {
        Self { keychain_id }
    }

    pub async fn check_or_persist_descriptor_checksum(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain: impl Into<KeychainKindPg>,
        script_bytes: &[u8],
    ) -> Result<(), bdk::Error> {
        let kind = keychain.into();
        let record = sqlx::query!(
            r#"SELECT script_bytes
            FROM descriptor_checksums WHERE keychain_id = $1 AND keychain_kind = $2"#,
            Uuid::from(self.keychain_id),
            kind as KeychainKindPg
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        if let Some(record) = record.get(0) {
            return if script_bytes == &record.script_bytes {
                Ok(())
            } else {
                Err(bdk::Error::ChecksumMismatch)
            };
        } else {
            sqlx::query!(
                r#"INSERT INTO descriptor_checksums (script_bytes, keychain_kind, keychain_id)
            VALUES ($1, $2, $3)"#,
                script_bytes,
                kind as KeychainKindPg,
                Uuid::from(self.keychain_id),
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| bdk::Error::Generic(e.to_string()))?;
            Ok(())
        }
    }
}
