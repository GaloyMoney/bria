use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use super::convert::BdkKeychainKind;
use crate::primitives::{bitcoin::ScriptBuf, *};

pub struct ScriptPubkeys {
    keychain_id: KeychainId,
    pool: PgPool,
}

impl ScriptPubkeys {
    pub fn new(keychain_id: KeychainId, pool: PgPool) -> Self {
        Self { keychain_id, pool }
    }

    pub async fn persist_all(
        &self,
        keys: Vec<(BdkKeychainKind, u32, ScriptBuf)>,
    ) -> Result<(), bdk::Error> {
        const BATCH_SIZE: usize = 5000;
        let chunks = keys.chunks(BATCH_SIZE);
        for chunk in chunks {
            let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
                r#"INSERT INTO bdk_script_pubkeys
        (keychain_id, keychain_kind, path, script, script_hex, script_fmt)"#,
            );

            query_builder.push_values(chunk, |mut builder, (keychain, path, script)| {
                builder.push_bind(self.keychain_id);
                builder.push_bind(keychain);
                builder.push_bind(*path as i32);
                builder.push_bind(script.as_bytes());
                builder.push_bind(format!("{:02x}", script));
                builder.push_bind(format!("{:?}", script));
            });
            query_builder.push("ON CONFLICT DO NOTHING");

            let query = query_builder.build();
            query
                .execute(&self.pool)
                .await
                .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn find_script(
        &self,
        keychain: impl Into<BdkKeychainKind>,
        path: u32,
    ) -> Result<Option<ScriptBuf>, bdk::Error> {
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
        Ok(rows
            .into_iter()
            .next()
            .map(|row| ScriptBuf::from(row.script)))
    }

    pub async fn find_path(
        &self,
        script: &ScriptBuf,
    ) -> Result<Option<(BdkKeychainKind, u32)>, bdk::Error> {
        let rows = sqlx::query!(
            r#"SELECT keychain_kind as "keychain_kind: BdkKeychainKind", path FROM bdk_script_pubkeys
            WHERE keychain_id = $1 AND script_hex = ENCODE($2, 'hex')"#,
            Uuid::from(self.keychain_id),
            script.as_bytes(),
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
    ) -> Result<Vec<ScriptBuf>, bdk::Error> {
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
                        Some(ScriptBuf::from(row.script))
                    } else {
                        None
                    }
                } else {
                    Some(ScriptBuf::from(row.script))
                }
            })
            .collect())
    }
}
