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
            (keychain_id, keychain_kind, path, script, script_hex, script_fmt)
            VALUES ($1, $2, $3, $4, ENCODE($4, 'hex'), $5) ON CONFLICT DO NOTHING"#,
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

    pub async fn find_script(
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

    pub async fn find_path(
        &self,
        script: &Script,
    ) -> Result<Option<(BdkKeychainKind, u32)>, bdk::Error> {
        let rows = sqlx::query!(
            r#"SELECT keychain_kind as "keychain_kind: BdkKeychainKind", path FROM bdk_script_pubkeys
            WHERE keychain_id = $1 AND script_hex = ENCODE($2, 'hex')"#,
            Uuid::from(self.keychain_id),
            script.as_ref(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        if let Some(row) = rows.into_iter().next() {
            Ok(Some((row.keychain_kind, row.path as u32)))
        } else {
            Ok(None)
        }
    }

    pub async fn list_scripts(
        &self,
        keychain: Option<impl Into<BdkKeychainKind>>,
    ) -> Result<Vec<Script>, bdk::Error> {
        let kind = keychain.map(|k| k.into());
        let rows = sqlx::query!(
            r#"SELECT script, keychain_kind as "keychain_kind: BdkKeychainKind" FROM bdk_script_pubkeys
            WHERE keychain_id = $1"#,
            Uuid::from(self.keychain_id),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                if let Some(kind) = kind {
                    if kind == row.keychain_kind {
                        Some(Script::from(row.script))
                    } else {
                        None
                    }
                } else {
                    Some(Script::from(row.script))
                }
            })
            .collect())
    }
}
