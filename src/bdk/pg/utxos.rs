use bdk::{LocalUtxo, TransactionDetails};
use sqlx::{PgPool, Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

use crate::{error::*, primitives::*};

pub struct UnsyncedIncomeUtxo {
    pub local_utxo: LocalUtxo,
    pub path: u32,
    pub confirmation_time: Option<bitcoin::BlockTime>,
}

pub struct ConfirmedIncomeUtxo {
    pub outpoint: bitcoin::OutPoint,
    pub spent: bool,
    pub confirmation_time: bitcoin::BlockTime,
}

pub struct Utxos {
    pool: PgPool,
}

impl Utxos {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn persist(
        &self,
        keychain_id: KeychainId,
        utxo: &LocalUtxo,
    ) -> Result<(), bdk::Error> {
        sqlx::query!(
            r#"INSERT INTO bdk_utxos (keychain_id, tx_id, vout, utxo_json, is_spent)
            VALUES ($1, $2, $3, $4, $5) ON CONFLICT (keychain_id, tx_id, vout)
            DO UPDATE SET utxo_json = EXCLUDED.utxo_json, is_spent = $5, modified_at = NOW()"#,
            Uuid::from(keychain_id),
            utxo.outpoint.txid.to_string(),
            utxo.outpoint.vout as i32,
            serde_json::to_value(utxo)?,
            utxo.is_spent,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(())
    }

    pub async fn list_local_utxos(
        &self,
        keychain_id: KeychainId,
    ) -> Result<Vec<LocalUtxo>, bdk::Error> {
        let utxos = sqlx::query!(
            r#"SELECT utxo_json FROM bdk_utxos WHERE keychain_id = $1"#,
            Uuid::from(keychain_id),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(utxos
            .into_iter()
            .map(|utxo| serde_json::from_value(utxo.utxo_json).expect("Could not deserialize utxo"))
            .collect())
    }

    #[instrument(name = "bdk_utxos.find_unsynced_income_utxo", skip(self, tx))]
    pub async fn find_unsynced_income_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
    ) -> Result<Option<UnsyncedIncomeUtxo>, BriaError> {
        let row = sqlx::query!(
            r#"WITH updated_utxo AS (
            UPDATE bdk_utxos SET synced_to_bria = true, modified_at = NOW()
            WHERE keychain_id = $1 AND (tx_id, vout) IN (
                SELECT tx_id, vout FROM bdk_utxos
                WHERE keychain_id = $1 AND synced_to_bria = false AND utxo_json->>'keychain' = 'External'
                ORDER BY created_at
                LIMIT 1
            )
            RETURNING tx_id, utxo_json
            )
            SELECT utxo_json, path, details_json
            FROM updated_utxo u
            JOIN bdk_script_pubkeys p
            ON p.keychain_id = $1 AND u.utxo_json->'txout'->>'script_pubkey' = p.script_hex
            JOIN bdk_transactions t ON u.tx_id = t.tx_id"#,
            Uuid::from(keychain_id),
        )
        .fetch_optional(tx)
        .await?;

        Ok(row.map(|row| {
            let local_utxo: LocalUtxo =
                serde_json::from_value(row.utxo_json).expect("Could not deserialize utxo_json");
            UnsyncedIncomeUtxo {
                local_utxo,
                path: row.path as u32,
                confirmation_time: serde_json::from_value::<TransactionDetails>(row.details_json)
                    .expect("Could not deserialize transaction details")
                    .confirmation_time,
            }
        }))
    }

    #[instrument(name = "bdk_utxos.find_settled_income_utxo", skip(self, tx))]
    pub async fn find_settled_income_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        min_height: u32,
    ) -> Result<Option<ConfirmedIncomeUtxo>, BriaError> {
        let row = sqlx::query!(
            r#"WITH updated_utxo AS (
            UPDATE bdk_utxos SET confirmation_synced_to_bria = true, modified_at = NOW()
            WHERE keychain_id = $1 AND (tx_id, vout) IN (
                SELECT u.tx_id, vout
                FROM bdk_utxos u
                JOIN bdk_transactions t
                ON u.keychain_id = t.keychain_id AND u.tx_id = t.tx_id
                WHERE u.keychain_id = $1
                AND utxo_json->>'keychain' = 'External'
                AND synced_to_bria = true
                AND confirmation_synced_to_bria = false
                AND (details_json->'confirmation_time'->'height')::INTEGER <= $2
                ORDER BY created_at
                LIMIT 1
            )
            RETURNING tx_id, utxo_json
            )
            SELECT u.tx_id, utxo_json, details_json
            FROM updated_utxo u JOIN bdk_transactions t on u.tx_id = t.tx_id"#,
            Uuid::from(keychain_id),
            min_height as i32,
        )
        .fetch_optional(tx)
        .await?;

        Ok(row.map(|row| {
            let local_utxo = serde_json::from_value::<LocalUtxo>(row.utxo_json)
                .expect("Could not deserialize utxo");
            let tx_details = serde_json::from_value::<TransactionDetails>(row.details_json)
                .expect("Could not deserialize tx details");
            ConfirmedIncomeUtxo {
                outpoint: local_utxo.outpoint,
                spent: local_utxo.is_spent,
                confirmation_time: tx_details
                    .confirmation_time
                    .expect("query should always return confirmation_time"),
            }
        }))
    }
}
