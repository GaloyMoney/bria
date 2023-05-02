use bdk::{LocalUtxo, TransactionDetails};
use sqlx::{PgPool, Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

use crate::{error::*, primitives::*};

pub struct ConfirmedIncomeUtxo {
    pub outpoint: bitcoin::OutPoint,
    pub spent: bool,
    pub confirmation_time: bitcoin::BlockTime,
}

pub struct Utxos {
    keychain_id: KeychainId,
    pool: PgPool,
}

impl Utxos {
    pub fn new(keychain_id: KeychainId, pool: PgPool) -> Self {
        Self { keychain_id, pool }
    }

    pub async fn persist(&self, utxo: &LocalUtxo) -> Result<(), bdk::Error> {
        sqlx::query!(
            r#"INSERT INTO bdk_utxos (keychain_id, tx_id, vout, utxo_json, is_spent)
            VALUES ($1, $2, $3, $4, $5) ON CONFLICT (keychain_id, tx_id, vout)
            DO UPDATE SET utxo_json = EXCLUDED.utxo_json, is_spent = $5, modified_at = NOW()"#,
            Uuid::from(self.keychain_id),
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

    pub async fn list_local_utxos(&self) -> Result<Vec<LocalUtxo>, bdk::Error> {
        let utxos = sqlx::query!(
            r#"SELECT utxo_json FROM bdk_utxos WHERE keychain_id = $1"#,
            Uuid::from(self.keychain_id),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(utxos
            .into_iter()
            .map(|utxo| serde_json::from_value(utxo.utxo_json).expect("Could not deserialize utxo"))
            .collect())
    }

    #[instrument(name = "bdk_utxos.mark_as_synced", skip(self, tx))]
    pub async fn mark_as_synced(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        utxo: &LocalUtxo,
    ) -> Result<(), BriaError> {
        sqlx::query!(
            r#"UPDATE bdk_utxos SET synced_to_bria = true, modified_at = NOW()
            WHERE keychain_id = $1 AND tx_id = $2 AND vout = $3"#,
            Uuid::from(self.keychain_id),
            utxo.outpoint.txid.to_string(),
            utxo.outpoint.vout as i32,
        )
        .execute(&mut *tx)
        .await?;
        Ok(())
    }

    #[instrument(name = "bdk_utxos.mark_confirmed", skip(self, tx))]
    pub async fn mark_confirmed(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        utxo: &LocalUtxo,
    ) -> Result<(), BriaError> {
        sqlx::query!(
            r#"UPDATE bdk_utxos SET confirmation_synced_to_bria = true, modified_at = NOW()
            WHERE keychain_id = $1 AND tx_id = $2 AND vout = $3"#,
            Uuid::from(self.keychain_id),
            utxo.outpoint.txid.to_string(),
            utxo.outpoint.vout as i32,
        )
        .execute(&mut *tx)
        .await?;
        Ok(())
    }

    #[instrument(name = "bdk_utxos.find_confirmed_income_utxo", skip(self, tx))]
    pub async fn find_confirmed_income_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
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
                AND u.synced_to_bria = true
                AND u.confirmation_synced_to_bria = false
                AND (details_json->'confirmation_time'->'height')::INTEGER <= $2
                ORDER BY t.height ASC NULLS LAST
                LIMIT 1
            )
            RETURNING tx_id, utxo_json
            )
            SELECT u.tx_id, utxo_json, details_json
            FROM updated_utxo u JOIN bdk_transactions t on u.tx_id = t.tx_id"#,
            Uuid::from(self.keychain_id),
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