use bdk::{bitcoin::blockdata::transaction::OutPoint, BlockTime, LocalUtxo, TransactionDetails};
use sqlx::{PgPool, Postgres, QueryBuilder, Row, Transaction};
use std::collections::{HashMap, HashSet};
use tracing::instrument;
use uuid::Uuid;

use crate::{error::*, primitives::*};

pub struct SettledUtxo {
    pub settled_id: Uuid,
    pub pending_id: Uuid,
    pub confirmation_time: BlockTime,
    pub local_utxo: LocalUtxo,
}

pub struct PendingUtxo {
    pub pending_id: Uuid,
    pub confirmation_time: Option<BlockTime>,
    pub local_utxo: LocalUtxo,
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

    #[instrument(name = "utxos.find_new_pending", skip_all)]
    pub async fn find_new_pending_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<PendingUtxo>, BriaError> {
        let pending_id = Uuid::new_v4();
        let utxos = sqlx::query!(
            r#"WITH utxo AS (
                 UPDATE bdk_utxos SET ledger_tx_pending_id = $1
                 WHERE keychain_id = $2 AND (tx_id, vout) in (
                   SELECT tx_id, vout FROM bdk_utxos
                   WHERE keychain_id = $2 AND ledger_tx_pending_id IS NULL LIMIT 1)
                 RETURNING tx_id, utxo_json
                 )
               SELECT u.tx_id, utxo_json, details_json
                 FROM utxo u
                 JOIN bdk_transactions t ON u.tx_id = t.tx_id"#,
            pending_id,
            Uuid::from(self.keychain_id),
        )
        .fetch_optional(&mut *tx)
        .await?;
        Ok(utxos.map(|utxo| PendingUtxo {
            pending_id,
            local_utxo: serde_json::from_value(utxo.utxo_json).expect("Could not deserialize utxo"),
            confirmation_time: serde_json::from_value::<TransactionDetails>(utxo.details_json)
                .expect("Could not deserialize transaction details")
                .confirmation_time,
        }))
    }

    #[instrument(name = "utxos.find_new_settled", skip(self, tx))]
    pub async fn find_new_settled_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        utxos_to_skip: &Vec<OutPoint>,
    ) -> Result<Option<SettledUtxo>, BriaError> {
        let skip_utxos = utxos_to_skip
            .iter()
            .map(|OutPoint { txid, vout }| format!("{txid}:{vout}"))
            .collect::<Vec<_>>();
        let settled_id = Uuid::new_v4();
        let utxos = sqlx::query!(
            r#"WITH utxo AS (
                 UPDATE bdk_utxos SET ledger_tx_settled_id = $1
                 WHERE keychain_id = $2 AND (tx_id, vout) in (
                   SELECT u.tx_id, vout
                     FROM bdk_utxos u
                     JOIN bdk_transactions t
                       ON u.keychain_id = t.keychain_id
                       AND u.tx_id = t.tx_id
                   WHERE u.keychain_id = $2
                   AND ledger_tx_settled_id IS NULL
                   AND ledger_tx_pending_id IS NOT NULL
                   AND (details_json->'confirmation_time'->'height')::INTEGER > 0
                   AND NOT (u.tx_id || ':' || vout::TEXT = ANY($3))
                   LIMIT 1)
                 RETURNING tx_id, utxo_json, ledger_tx_pending_id
               )
               SELECT u.tx_id, utxo_json, ledger_tx_pending_id AS "ledger_tx_pending_id!", details_json
               FROM utxo u JOIN bdk_transactions t on u.tx_id = t.tx_id"#,
            settled_id,
            Uuid::from(self.keychain_id),
            &skip_utxos[..],
        )
        .fetch_optional(&mut *tx)
        .await?;
        Ok(utxos.map(|utxo| SettledUtxo {
            settled_id,
            pending_id: utxo.ledger_tx_pending_id,
            local_utxo: serde_json::from_value(utxo.utxo_json).expect("Could not deserialize utxo"),
            confirmation_time: serde_json::from_value::<TransactionDetails>(utxo.details_json)
                .expect("Could not deserialize tx details")
                .confirmation_time
                .expect("Query should only return confirmed transactions"),
        }))
    }

    #[instrument(name = "utxos.list_reserved_unspent_utxos", skip(self, tx))]
    pub async fn list_reserved_unspent_utxos(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ids: HashSet<KeychainId>,
    ) -> Result<HashMap<KeychainId, Vec<OutPoint>>, BriaError> {
        let uuids = ids.into_iter().map(Uuid::from).collect::<Vec<_>>();
        let rows = sqlx::query!(
            r#"SELECT keychain_id, utxo_json
                          FROM bdk_utxos
                          WHERE keychain_id = ANY($1)
                            AND (utxo_json->'is_spent')::BOOLEAN = false
                            AND spending_batch_id IS NOT NULL FOR UPDATE"#,
            &uuids[..]
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut utxos = HashMap::new();
        for row in rows {
            let keychain_id = KeychainId::from(row.keychain_id);
            let utxos: &mut Vec<_> = utxos.entry(keychain_id).or_default();
            let details: LocalUtxo =
                serde_json::from_value(row.utxo_json).expect("Couldn't deserialize utxo_details");
            utxos.push(details.outpoint);
        }
        Ok(utxos)
    }

    #[instrument(name = "utxos.reserve_utxos", skip(self, tx, utxos))]
    pub async fn reserve_utxos(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        batch_id: BatchId,
        utxos: impl Iterator<Item = (KeychainId, OutPoint)>,
    ) -> Result<(), BriaError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"UPDATE bdk_utxos
            SET spending_batch_id = "#,
        );
        query_builder.push_bind(Uuid::from(batch_id));
        query_builder.push("WHERE (keychain_id, tx_id, vout) IN");
        query_builder.push_tuples(
            utxos.map(|(keychain_id, utxo)| {
                (
                    Uuid::from(keychain_id),
                    utxo.txid.to_string(),
                    utxo.vout as i32,
                )
            }),
            |mut builder, (keychain_id, tx_id, vout)| {
                builder.push_bind(keychain_id);
                builder.push_bind(tx_id.to_string());
                builder.push_bind(vout);
            },
        );

        let query = query_builder.build();
        query.execute(&mut *tx).await?;
        Ok(())
    }

    #[instrument(name = "utxos.get_settled_utxos", skip(self))]
    pub async fn get_settled_utxos(
        &self,
        utxos: &HashMap<KeychainId, Vec<OutPoint>>,
    ) -> Result<Vec<SettledUtxo>, BriaError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT ledger_tx_pending_id, ledger_tx_settled_id, utxo_json, details_json
            FROM bdk_utxos
            WHERE (keychain_id, tx_id, vout) IN"#,
        );

        query_builder.push_tuples(
            utxos.iter().flat_map(|(keychain_id, utxos)| {
                utxos.iter().map(move |utxo| {
                    (
                        Uuid::from(*keychain_id),
                        utxo.txid.to_string(),
                        utxo.vout as i32,
                    )
                })
            }),
            |mut builder, (keychain_id, tx_id, vout)| {
                builder.push_bind(keychain_id);
                builder.push_bind(tx_id.to_string());
                builder.push_bind(vout);
            },
        );

        let query = query_builder.build();
        let rows = query.fetch_all(&self.pool).await?;

        Ok(rows
            .into_iter()
            .map(|row| SettledUtxo {
                pending_id: row.get::<Uuid, _>("ledger_tx_pending_id"),
                settled_id: row.get::<Uuid, _>("ledger_tx_settled_id"),
                local_utxo: serde_json::from_value(row.get("utxo_json"))
                    .expect("Could not deserialize utxo"),
                confirmation_time: serde_json::from_value::<TransactionDetails>(
                    row.get("details_json"),
                )
                .expect("Could not deserialize tx details")
                .confirmation_time
                .expect("Query should only return confirmed transactions"),
            })
            .collect())
    }
}
