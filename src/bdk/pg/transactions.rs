use bdk::{bitcoin::Txid, LocalUtxo, TransactionDetails};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::{error::*, primitives::*};

pub struct UnsyncedTransaction {
    pub tx_id: bitcoin::Txid,
    pub confirmation_time: Option<bitcoin::BlockTime>,
    pub sats_per_vbyte_when_created: f32,
    pub inputs: Vec<(LocalUtxo, u32)>,
    pub outputs: Vec<(LocalUtxo, u32)>,
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
    pub async fn find_unsynced_tx(&self) -> Result<Option<UnsyncedTransaction>, BriaError> {
        let rows = sqlx::query!(
        r#"WITH tx_to_sync AS (
           SELECT tx_id, details_json, height
           FROM bdk_transactions
           WHERE keychain_id = $1 AND synced_to_bria = false
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
           JOIN tx_to_sync t ON CONCAT(u.tx_id, ':', u.vout::text) = ANY(ARRAY(
               SELECT output FROM previous_outputs
           )) OR u.tx_id = t.tx_id
           JOIN bdk_script_pubkeys p
           ON p.keychain_id = $1 AND u.utxo_json->'txout'->>'script_pubkey' = p.script_hex
           WHERE u.keychain_id = $1 AND u.synced_to_bria = false
        "#,
        Uuid::from(self.keychain_id),
        )
           .fetch_all(&self.pool)
           .await?;

        tracing::Span::current().record("n_rows", rows.len());

        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut tx_id = None;
        let mut confirmation_time = None;
        let mut sats_per_vbyte_when_created = 0.0;

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
                sats_per_vbyte_when_created = details.fee.expect("Fee") as f32
                    / details.transaction.expect("transaction").vsize() as f32;
                confirmation_time = details.confirmation_time;
            }
        }
        Ok(tx_id.map(|tx_id| UnsyncedTransaction {
            tx_id,
            confirmation_time,
            sats_per_vbyte_when_created,
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
