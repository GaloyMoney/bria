use bdk::{bitcoin::Txid, LocalUtxo, TransactionDetails};
use sqlx::{PgPool, Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

use crate::{error::*, primitives::*};

#[derive(Debug)]
pub struct UnsyncedTransaction {
    pub tx_id: bitcoin::Txid,
    pub confirmation_time: Option<bitcoin::BlockTime>,
    pub sats_per_vbyte_when_created: f32,
    pub total_utxo_in_sats: Satoshis,
    pub fee_sats: Satoshis,
    pub inputs: Vec<(LocalUtxo, u32)>,
    pub outputs: Vec<(LocalUtxo, u32)>,
}

pub struct ConfirmedSpendTransaction {
    pub tx_id: bitcoin::Txid,
    pub confirmation_time: bitcoin::BlockTime,
    pub inputs: Vec<LocalUtxo>,
    pub outputs: Vec<LocalUtxo>,
}

pub struct Transactions {
    keychain_id: KeychainId,
    pool: PgPool,
}

impl Transactions {
    pub fn new(keychain_id: KeychainId, pool: PgPool) -> Self {
        Self { keychain_id, pool }
    }

    pub async fn persist(&self, tx: &TransactionDetails) -> Result<(), bdk::Error> {
        sqlx::query!(
            r#"
        INSERT INTO bdk_transactions (keychain_id, tx_id, details_json, sent, height)
        VALUES ($1, $2, $3, $4, $5) ON CONFLICT (keychain_id, tx_id)
        DO UPDATE SET details_json = EXCLUDED.details_json, height = $5, modified_at = NOW()"#,
            Uuid::from(self.keychain_id),
            tx.txid.to_string(),
            serde_json::to_value(&tx)?,
            tx.sent as i64,
            tx.confirmation_time.as_ref().map(|t| t.height as i32),
        )
        .execute(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(())
    }

    pub async fn find_by_id(&self, tx_id: &Txid) -> Result<Option<TransactionDetails>, bdk::Error> {
        let tx = sqlx::query!(
            r#"
        SELECT details_json FROM bdk_transactions WHERE keychain_id = $1 AND tx_id = $2"#,
            Uuid::from(self.keychain_id),
            tx_id.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(tx.map(|tx| serde_json::from_value(tx.details_json).unwrap()))
    }

    pub async fn list(&self) -> Result<Vec<TransactionDetails>, bdk::Error> {
        let txs = sqlx::query!(
            r#"
        SELECT details_json FROM bdk_transactions WHERE keychain_id = $1"#,
            Uuid::from(self.keychain_id),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| bdk::Error::Generic(e.to_string()))?;
        Ok(txs
            .into_iter()
            .map(|tx| serde_json::from_value(tx.details_json).unwrap())
            .collect())
    }

    #[instrument(name = "bdk_transactions.find_unsynced_tx", skip(self), fields(n_rows))]
    pub async fn find_unsynced_tx(
        &self,
        excluded_tx_ids: &[String],
    ) -> Result<Option<UnsyncedTransaction>, BriaError> {
        let rows = sqlx::query!(
        r#"WITH tx_to_sync AS (
           SELECT tx_id, details_json, height
           FROM bdk_transactions
           WHERE keychain_id = $1 AND synced_to_bria = false AND tx_id != ALL($2)
           ORDER BY (details_json->'confirmation_time'->'height') ASC NULLS LAST
           LIMIT 1
           ),
           previous_outputs AS (
               SELECT (jsonb_array_elements(details_json->'transaction'->'input')->>'previous_output') AS output
               FROM tx_to_sync
           )
           SELECT t.tx_id, details_json, utxo_json, path, vout,
                  CASE WHEN u.tx_id = t.tx_id THEN true ELSE false END AS "is_tx_output!"
           FROM bdk_utxos u
           JOIN tx_to_sync t ON u.tx_id = t.tx_id OR CONCAT(u.tx_id, ':', u.vout::text) = ANY(
               SELECT output FROM previous_outputs
           ) OR u.tx_id = t.tx_id
           JOIN bdk_script_pubkeys p
           ON p.keychain_id = $1 AND u.utxo_json->'txout'->>'script_pubkey' = p.script_hex
           WHERE u.keychain_id = $1 AND (u.synced_to_bria = false OR u.tx_id != t.tx_id)
        "#,
        Uuid::from(self.keychain_id),
        &excluded_tx_ids
        )
           .fetch_all(&self.pool)
           .await?;

        tracing::Span::current().record("n_rows", rows.len());

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut tx_id = None;
        let mut confirmation_time = None;
        let mut sats_per_vbyte_when_created = 0.0;

        let mut total_utxo_in_sats = Satoshis::ZERO;
        let mut fee_sats = Satoshis::ZERO;

        for row in rows {
            let utxo: LocalUtxo = serde_json::from_value(row.utxo_json)?;
            if row.is_tx_output {
                outputs.push((utxo, row.path as u32));
            } else {
                inputs.push((utxo, row.path as u32));
            }
            if tx_id.is_none() {
                tx_id = Some(row.tx_id.parse().expect("couldn't parse tx_id"));
                let details: TransactionDetails = serde_json::from_value(row.details_json)?;
                total_utxo_in_sats = Satoshis::from(details.sent);
                fee_sats = Satoshis::from(details.fee.expect("Fee"));
                sats_per_vbyte_when_created = details.fee.expect("Fee") as f32
                    / details.transaction.expect("transaction").vsize() as f32;
                confirmation_time = details.confirmation_time;
            }
        }
        Ok(tx_id.map(|tx_id| UnsyncedTransaction {
            tx_id,
            total_utxo_in_sats,
            fee_sats,
            confirmation_time,
            sats_per_vbyte_when_created,
            inputs,
            outputs,
        }))
    }

    #[instrument(
        name = "bdk_transactions.find_confirmed_spend_tx",
        skip(self),
        fields(n_rows)
    )]
    pub async fn find_confirmed_spend_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        min_height: u32,
    ) -> Result<Option<ConfirmedSpendTransaction>, BriaError> {
        let rows = sqlx::query!(r#"
            WITH tx_to_sync AS (
              UPDATE bdk_transactions SET confirmation_synced_to_bria = true, modified_at = NOW()
              WHERE keychain_id = $1 AND tx_id IN (
                SELECT tx_id
                FROM bdk_transactions
                WHERE keychain_id = $1
                AND (details_json->'confirmation_time'->'height')::INTEGER <= $2
                AND confirmation_synced_to_bria = false
                ORDER BY (details_json->'confirmation_time'->'height') ASC
                LIMIT 1)
                RETURNING tx_id, details_json
            ),
            previous_outputs AS (
                SELECT (jsonb_array_elements(details_json->'transaction'->'input')->>'previous_output') AS output
                FROM tx_to_sync
            )
            SELECT t.tx_id, details_json, utxo_json, vout,
                   CASE WHEN u.tx_id = t.tx_id THEN true ELSE false END AS "is_tx_output!"
            FROM bdk_utxos u
            JOIN tx_to_sync t ON u.tx_id = t.tx_id OR CONCAT(u.tx_id, ':', u.vout::text) = ANY(
                SELECT output FROM previous_outputs
            ) OR u.tx_id = t.tx_id
            WHERE u.keychain_id = $1 AND (u.confirmation_synced_to_bria = false OR u.tx_id != t.tx_id)
        "#,
            Uuid::from(self.keychain_id),
            min_height as i32
        )
        .fetch_all(tx)
        .await?;

        tracing::Span::current().record("n_rows", &rows.len());

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut tx_id = None;
        let mut confirmation_time = None;

        for row in rows {
            let utxo: LocalUtxo = serde_json::from_value(row.utxo_json)?;
            if row.is_tx_output {
                outputs.push(utxo);
            } else {
                inputs.push(utxo);
            }
            if tx_id.is_none() {
                tx_id = Some(row.tx_id.parse().expect("couldn't parse tx_id"));
                let details: TransactionDetails = serde_json::from_value(row.details_json)?;
                confirmation_time = details.confirmation_time;
            }
        }

        Ok(tx_id.map(|tx_id| ConfirmedSpendTransaction {
            tx_id,
            confirmation_time: confirmation_time
                .expect("query should always return confirmation_time"),
            inputs,
            outputs,
        }))
    }

    #[instrument(name = "bdk_transactions.mark_as_synced", skip(self))]
    pub async fn mark_as_synced(&self, tx_id: bitcoin::Txid) -> Result<(), BriaError> {
        sqlx::query!(
            r#"UPDATE bdk_transactions SET synced_to_bria = true, modified_at = NOW()
            WHERE keychain_id = $1 AND tx_id = $2"#,
            Uuid::from(self.keychain_id),
            tx_id.to_string(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
