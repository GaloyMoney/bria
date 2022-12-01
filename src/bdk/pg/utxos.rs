use bdk::{bitcoin::Txid, LocalUtxo};
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use sqlx_ledger::TransactionId as LedgerTransactionId;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{error::*, primitives::*};

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
            r#"INSERT INTO bdk_utxos (keychain_id, tx_id, vout, utxo_json)
            VALUES ($1, $2, $3, $4) ON CONFLICT (keychain_id, tx_id, vout)
            DO UPDATE set utxo_json = EXCLUDED.utxo_json"#,
            Uuid::from(self.keychain_id),
            utxo.outpoint.txid.to_string(),
            utxo.outpoint.vout as i32,
            serde_json::to_value(utxo)?,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<LocalUtxo>, bdk::Error> {
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

    pub async fn list_without_pending_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Vec<LocalUtxo>, BriaError> {
        let utxos = sqlx::query!(
            r#"SELECT utxo_json FROM bdk_utxos
            WHERE keychain_id = $1 AND ledger_tx_pending_id IS NULL FOR UPDATE"#,
            Uuid::from(self.keychain_id),
        )
        .fetch_all(&mut *tx)
        .await?;
        Ok(utxos
            .into_iter()
            .map(|utxo| serde_json::from_value(utxo.utxo_json).expect("Could not deserialize utxo"))
            .collect())
    }

    pub async fn set_pending_tx_id(
        &self,
        ids: Vec<(Txid, u32, LedgerTransactionId)>,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), BriaError> {
        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(r#"UPDATE bdk_utxos SET ledger_tx_pending_id = CASE"#);
        let mut bind_numbers = HashMap::new();
        let mut next_bind_number = 1;
        for (tx_id, vout, ledger_tx_pending_id) in ids {
            bind_numbers.insert((tx_id, vout), next_bind_number);
            next_bind_number += 3;
            query_builder.push(" WHEN tx_id = ");
            query_builder.push_bind(tx_id.to_string());
            query_builder.push(" AND vout = ");
            query_builder.push_bind(vout as i32);
            query_builder.push(" THEN ");
            query_builder.push_bind(Uuid::from(ledger_tx_pending_id));
        }
        query_builder.push(" END WHERE (tx_id, vout) IN");
        query_builder.push_tuples(bind_numbers, |mut builder, (_, n)| {
            builder.push(format!("${}, ${}", n, n + 1));
        });
        query_builder.build().execute(&mut *tx).await?;
        Ok(())
    }

    pub async fn list_without_settled_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Vec<LocalUtxo>, BriaError> {
        let utxos = sqlx::query!(
            r#"SELECT utxo_json FROM bdk_utxos
            WHERE keychain_id = $1 AND ledger_tx_settled_id IS NULL FOR UPDATE"#,
            Uuid::from(self.keychain_id),
        )
        .fetch_all(&mut *tx)
        .await?;
        Ok(utxos
            .into_iter()
            .map(|utxo| serde_json::from_value(utxo.utxo_json).expect("Could not deserialize utxo"))
            .collect())
    }

    pub async fn set_settled_tx_id(
        &self,
        ids: Vec<(Txid, u32, LedgerTransactionId)>,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), BriaError> {
        let mut query_builder: QueryBuilder<Postgres> =
            QueryBuilder::new(r#"UPDATE bdk_utxos SET ledger_tx_settled_id = CASE"#);
        let mut bind_numbers = HashMap::new();
        let mut next_bind_number = 1;
        for (tx_id, vout, ledger_tx_pending_id) in ids {
            bind_numbers.insert((tx_id, vout), next_bind_number);
            next_bind_number += 3;
            query_builder.push(" WHEN tx_id = ");
            query_builder.push_bind(tx_id.to_string());
            query_builder.push(" AND vout = ");
            query_builder.push_bind(vout as i32);
            query_builder.push(" THEN ");
            query_builder.push_bind(Uuid::from(ledger_tx_pending_id));
        }
        query_builder.push(" END WHERE (tx_id, vout) IN");
        query_builder.push_tuples(bind_numbers, |mut builder, (_, n)| {
            builder.push(format!("${}, ${}", n, n + 1));
        });
        query_builder.build().execute(&mut *tx).await?;
        Ok(())
    }
}
