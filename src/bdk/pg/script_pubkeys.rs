use bitcoin::blockdata::script::Script;
use sqlx::PgPool;
use uuid::Uuid;

use super::convert::BdkKeychainKind;
use crate::primitives::*;

pub struct ScriptPubkeys {
    keychain_id: KeychainId,
    pool: PgPool,
}

impl ScriptPubkeys {
    pub fn new(keychain_id: KeychainId, pool: PgPool) -> Self {
        Self { keychain_id, pool }
    }

    pub async fn persist(
        &self,
        keychain: impl Into<BdkKeychainKind>,
        path: u32,
        script: &Script,
    ) -> Result<(), bdk::Error> {
        let kind = keychain.into();
        sqlx::query!(
            r#"INSERT INTO bdk_script_pubkeys
            (keychain_id, keychain_kind, path, script, script_fmt)
            VALUES ($1, $2, $3, $4, $5)"#,
            Uuid::from(self.keychain_id),
            kind as BdkKeychainKind,
            path as i32,
            script.as_ref(),
            format!("{:?}", script)
        )
        .execute(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(())
    }

    pub async fn find(
        &self,
        keychain: impl Into<BdkKeychainKind>,
        path: u32,
    ) -> Result<Option<Script>, bdk::Error> {
        let kind = keychain.into();
        let rows = sqlx::query!(
            r#"SELECT script FROM bdk_script_pubkeys
            WHERE keychain_id = $1 AND keychain_kind = $2 AND path = $3"#,
            Uuid::from(self.keychain_id),
            kind as BdkKeychainKind,
            path as i32,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(rows.into_iter().next().map(|row| Script::from(row.script)))
    }
}
