use bdk::{bitcoin::blockdata::transaction::OutPoint, LocalUtxo, TransactionDetails};
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use tracing::instrument;
use uuid::Uuid;

use crate::{bdk::error::BdkError, primitives::*};

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

    #[instrument(name = "bdk.utxos.persist_all", skip_all)]
    pub async fn persist_all(&self, utxos: Vec<LocalUtxo>) -> Result<(), bdk::Error> {
        const BATCH_SIZE: usize = 5000;
        let batches = utxos.chunks(BATCH_SIZE);

        for batch in batches {
            let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
                r#"INSERT INTO bdk_utxos
            (keychain_id, tx_id, vout, utxo_json, is_spent)"#,
            );

            query_builder.push_values(batch, |mut builder, utxo| {
                builder.push_bind(Uuid::from(self.keychain_id));
                builder.push_bind(utxo.outpoint.txid.to_string());
                builder.push_bind(utxo.outpoint.vout as i32);
                builder.push_bind(serde_json::to_value(utxo).unwrap());
                builder.push_bind(utxo.is_spent);
            });

            query_builder.push("ON CONFLICT (keychain_id, tx_id, vout) DO UPDATE SET utxo_json = EXCLUDED.utxo_json, is_spent = EXCLUDED.is_spent, modified_at = NOW(), deleted_at = NULL");

            let query = query_builder.build();
            query
                .execute(&self.pool)
                .await
                .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        }

        Ok(())
    }

    #[instrument(name = "bdk.utxos.delete", skip_all)]
    pub async fn delete(
        &self,
        outpoint: &bitcoin::OutPoint,
    ) -> Result<Option<LocalUtxo>, bdk::Error> {
        let row = sqlx::query!(
            r#"UPDATE bdk_utxos SET deleted_at = NOW()
                 WHERE keychain_id = $1 AND tx_id = $2 AND vout = $3
                 RETURNING utxo_json"#,
            self.keychain_id as KeychainId,
            outpoint.txid.to_string(),
            outpoint.vout as i32,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;

        Ok(row.map(|row| {
            serde_json::from_value::<LocalUtxo>(row.utxo_json).expect("Could not deserialize utxo")
        }))
    }

    #[instrument(name = "bdk.utxos.undelete", skip_all)]
    pub async fn undelete(&self, outpoint: bitcoin::OutPoint) -> Result<(), BdkError> {
        sqlx::query!(
            r#"UPDATE bdk_utxos SET deleted_at = NULL
                 WHERE keychain_id = $1 AND tx_id = $2 AND vout = $3"#,
            self.keychain_id as KeychainId,
            outpoint.txid.to_string(),
            outpoint.vout as i32,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[instrument(name = "bdk.utxos.find", skip_all)]
    pub async fn find(&self, outpoint: &OutPoint) -> Result<Option<LocalUtxo>, bdk::Error> {
        let utxo = sqlx::query!(
            r#"
            SELECT utxo_json
            FROM bdk_utxos
            WHERE keychain_id = $1
            AND deleted_at IS NULL
            AND tx_id = $2
            AND vout = $3"#,
            self.keychain_id as KeychainId,
            outpoint.txid.to_string(),
            outpoint.vout as i32,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;

        Ok(utxo.map(|utxo| {
            serde_json::from_value(utxo.utxo_json).expect("Could not deserialize utxo")
        }))
    }

    #[instrument(name = "bdk.utxos.list_local_utxos", skip_all)]
    pub async fn list_local_utxos(&self) -> Result<Vec<LocalUtxo>, bdk::Error> {
        let utxos = sqlx::query!(
            r#"SELECT utxo_json FROM bdk_utxos WHERE keychain_id = $1 AND deleted_at IS NULL"#,
            self.keychain_id as KeychainId,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(utxos
            .into_iter()
            .map(|utxo| serde_json::from_value(utxo.utxo_json).expect("Could not deserialize utxo"))
            .collect())
    }

    #[instrument(name = "bdk.utxos.mark_as_synced", skip(self, tx))]
    pub async fn mark_as_synced(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        utxo: &LocalUtxo,
    ) -> Result<(), BdkError> {
        sqlx::query!(
            r#"UPDATE bdk_utxos SET synced_to_bria = true, modified_at = NOW()
            WHERE keychain_id = $1 AND tx_id = $2 AND vout = $3"#,
            self.keychain_id as KeychainId,
            utxo.outpoint.txid.to_string(),
            utxo.outpoint.vout as i32,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    #[instrument(name = "bdk.utxos.mark_confirmed", skip(self, tx))]
    pub async fn mark_confirmed(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        utxo: &LocalUtxo,
    ) -> Result<(), BdkError> {
        sqlx::query!(
            r#"UPDATE bdk_utxos SET confirmation_synced_to_bria = true, modified_at = NOW()
            WHERE keychain_id = $1 AND tx_id = $2 AND vout = $3"#,
            self.keychain_id as KeychainId,
            utxo.outpoint.txid.to_string(),
            utxo.outpoint.vout as i32,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    #[instrument(name = "bdk.utxos.find_confirmed_income_utxo", skip(self, tx))]
    pub async fn find_confirmed_income_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        min_height: u32,
    ) -> Result<Option<ConfirmedIncomeUtxo>, BdkError> {
        let row = sqlx::query!(
            r#"WITH updated_utxo AS (
            UPDATE bdk_utxos SET confirmation_synced_to_bria = true, modified_at = NOW()
            WHERE keychain_id = $1 AND (tx_id, vout) IN (
                SELECT u.tx_id, vout
                FROM bdk_utxos u
                JOIN bdk_transactions t
                ON u.keychain_id = t.keychain_id AND u.tx_id = t.tx_id
                WHERE u.keychain_id = $1
                AND u.deleted_at IS NULL
                AND t.deleted_at IS NULL
                AND (utxo_json->>'keychain' = 'External' OR (utxo_json->>'keychain' = 'Internal' AND sent = 0))
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
            self.keychain_id as KeychainId,
            min_height as i32,
        )
        .fetch_optional(&mut **tx)
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

    #[instrument(name = "bdk.utxos.find_and_remove_soft_deleted_utxo", skip_all)]
    pub async fn find_and_remove_soft_deleted_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<(bitcoin::OutPoint, KeychainId)>, BdkError> {
        let row = sqlx::query!(
            r#"DELETE FROM bdk_utxos 
               WHERE keychain_id = $1 AND (tx_id, vout) IN (
                   SELECT tx_id, vout FROM bdk_utxos 
                   WHERE keychain_id = $1 AND deleted_at IS NOT NULL 
                   LIMIT 1
               ) 
               RETURNING keychain_id, utxo_json;"#,
            self.keychain_id as KeychainId,
        )
        .fetch_optional(&mut **tx)
        .await?;
        Ok(row.map(|row| {
            let local_utxo = serde_json::from_value::<LocalUtxo>(row.utxo_json)
                .expect("Could not deserialize the utxo");
            let keychain_id = KeychainId::from(row.keychain_id);
            (local_utxo.outpoint, keychain_id)
        }))
    }
}
