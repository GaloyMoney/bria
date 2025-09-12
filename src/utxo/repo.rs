use sqlx::{Pool, Postgres, QueryBuilder, Row, Transaction};
use uuid::Uuid;

use std::collections::{HashMap, HashSet};

use super::{cpfp::CpfpCandidate, entity::*, error::UtxoError};
use crate::primitives::{bitcoin::*, *};

pub struct ReservableUtxo {
    pub keychain_id: KeychainId,
    #[allow(dead_code)]
    pub income_address: bool,
    pub outpoint: OutPoint,
    pub spending_batch_id: Option<BatchId>,
    pub utxo_settled_ledger_tx_id: Option<LedgerTransactionId>,
}

#[derive(Clone)]
pub(super) struct UtxoRepo {
    pool: Pool<Postgres>,
}

impl UtxoRepo {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn persist_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        utxo: NewUtxo<'_>,
    ) -> Result<Option<LedgerTransactionId>, UtxoError> {
        let sats_per_vbyte = u64::from(utxo.origin_tx_fee) as f32 / utxo.origin_tx_vbytes as f32;
        let result = sqlx::query!(
            r#"INSERT INTO bria_utxos
               (account_id, wallet_id, keychain_id, tx_id, vout,
               sats_per_vbyte_when_created, self_pay, kind, address_idx, value, address,
               script_hex, income_detected_ledger_tx_id, bdk_spent, detected_block_height,
               origin_tx_batch_id, origin_tx_payout_queue_id, origin_tx_vbytes,
               origin_tx_fee, trusted_origin_tx_input_tx_ids)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20)
               ON CONFLICT (keychain_id, tx_id, vout) DO NOTHING"#,
          utxo.account_id as AccountId,
          utxo.wallet_id as WalletId,
          utxo.keychain_id as KeychainId,
          utxo.outpoint.txid.to_string(),
          utxo.outpoint.vout as i32,
          sats_per_vbyte,
          utxo.self_pay,
          pg::PgKeychainKind::from(utxo.kind) as pg::PgKeychainKind,
          utxo.address_idx as i32,
          utxo.value.into_inner(),
          utxo.address.to_string(),
          utxo.script_hex,
          utxo.utxo_detected_ledger_tx_id as LedgerTransactionId,
          utxo.bdk_spent,
          utxo.detected_block_height as i32,
          utxo.origin_tx_batch_id as Option<BatchId>,
          utxo.origin_tx_payout_queue_id as Option<PayoutQueueId>,
          utxo.origin_tx_vbytes as i64,
          u64::from(utxo.origin_tx_fee) as i64,
          utxo.origin_tx_trusted_input_tx_ids,
        )
        .execute(&mut **tx)
        .await?;

        Ok(if result.rows_affected() > 0 {
            Some(utxo.utxo_detected_ledger_tx_id)
        } else {
            None
        })
    }

    pub async fn mark_utxo_settled(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        outpoint: OutPoint,
        bdk_spent: bool,
        block_height: u32,
    ) -> Result<SettledUtxo, UtxoError> {
        let new_confirmed_ledger_tx_id = LedgerTransactionId::new();

        let row = sqlx::query!(
            r#"UPDATE bria_utxos
            SET bdk_spent = $1,
                block_height = $2,
                income_settled_ledger_tx_id = $3,
                modified_at = NOW()
            WHERE keychain_id = $4
              AND tx_id = $5
              AND vout = $6
            RETURNING address_idx, value, address, income_detected_ledger_tx_id, spend_detected_ledger_tx_id"#,
            bdk_spent,
            block_height as i32,
            new_confirmed_ledger_tx_id as LedgerTransactionId,
            keychain_id as KeychainId,
            outpoint.txid.to_string(),
            outpoint.vout as i32,
        )
        .fetch_one(&mut **tx)
        .await?;

        Ok(SettledUtxo {
            keychain_id,
            value: Satoshis::from(row.value),
            address: Address::parse_from_trusted_source(&row.address),
            utxo_detected_ledger_tx_id: LedgerTransactionId::from(row.income_detected_ledger_tx_id),
            utxo_settled_ledger_tx_id: new_confirmed_ledger_tx_id,
            spend_detected_ledger_tx_id: row
                .spend_detected_ledger_tx_id
                .map(LedgerTransactionId::from),
        })
    }

    pub async fn mark_spent(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        utxos: impl Iterator<Item = &OutPoint>,
        tx_id: LedgerTransactionId,
    ) -> Result<Vec<SpentUtxo>, UtxoError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"WITH updated AS ( UPDATE bria_utxos
            SET bdk_spent = true, modified_at = NOW(), spend_detected_ledger_tx_id = "#,
        );
        query_builder.push_bind(tx_id);
        query_builder
            .push("WHERE spend_detected_ledger_tx_id IS NULL AND (keychain_id, tx_id, vout) IN");
        let mut n_inputs = 0;
        query_builder.push_tuples(utxos, |mut builder, out| {
            n_inputs += 1;
            builder.push_bind(keychain_id);
            builder.push_bind(out.txid.to_string());
            builder.push_bind(out.vout as i32);
        });
        query_builder.push(
            r#"RETURNING tx_id, vout, value, kind, sats_per_vbyte_when_created, CASE WHEN income_settled_ledger_tx_id IS NOT NULL THEN value ELSE 0 END as settled_value )
            SELECT tx_id, vout, value,
                CASE WHEN settled_value != 0 THEN true ELSE false END as confirmed,
                CASE WHEN kind = 'internal' THEN true ELSE false END as change_address
                FROM updated ORDER BY settled_value DESC, sats_per_vbyte_when_created DESC"#
        );

        let query = query_builder.build();
        let res = query.fetch_all(&mut **tx).await?;
        Ok(if n_inputs == res.len() {
            res.into_iter()
                .map(|row| SpentUtxo {
                    outpoint: OutPoint {
                        txid: row.get::<String, _>("tx_id").parse().unwrap(),
                        vout: row.get::<i32, _>("vout") as u32,
                    },
                    value: Satoshis::from(row.get::<rust_decimal::Decimal, _>("value")),
                    confirmed: row.get("confirmed"),
                    change_address: row.get("change_address"),
                })
                .collect()
        } else {
            Vec::new()
        })
    }

    pub async fn settle_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        utxos: impl Iterator<Item = &OutPoint>,
        tx_id: LedgerTransactionId,
    ) -> Result<Option<LedgerTransactionId>, UtxoError> {
        let keychain_id = Uuid::from(keychain_id);
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"UPDATE bria_utxos
            SET modified_at = NOW(), spend_settled_ledger_tx_id = "#,
        );
        query_builder.push_bind(tx_id);
        query_builder
            .push(" WHERE spend_settled_ledger_tx_id IS NULL AND (keychain_id, tx_id, vout) IN");
        let mut rows = 0;
        query_builder.push_tuples(utxos, |mut builder, out| {
            rows += 1;
            builder.push_bind(keychain_id);
            builder.push_bind(out.txid.to_string());
            builder.push_bind(out.vout as i32);
        });
        query_builder.push("RETURNING spend_detected_ledger_tx_id");
        let query = query_builder.build();
        let res = query.fetch_all(&mut **tx).await?;
        Ok(if rows == res.len() {
            Some(LedgerTransactionId::from(
                res[0].get::<Uuid, _>("spend_detected_ledger_tx_id"),
            ))
        } else {
            None
        })
    }

    pub async fn find_keychain_utxos(
        &self,
        keychain_ids: impl Iterator<Item = KeychainId>,
    ) -> Result<HashMap<KeychainId, KeychainUtxos>, UtxoError> {
        let keychain_ids: Vec<Uuid> = keychain_ids.map(Uuid::from).collect();
        let rows = sqlx::query!(
            r#"SELECT wallet_id, keychain_id, tx_id, vout, kind as "kind: pg::PgKeychainKind", address_idx, value, address, bdk_spent,
                  CASE
                      WHEN kind = 'external' THEN address
                      ELSE NULL
                  END as optional_address,
                  block_height, income_detected_ledger_tx_id, income_settled_ledger_tx_id, spending_batch_id
           FROM bria_utxos
           WHERE keychain_id = ANY($1) AND bdk_spent = false
           ORDER BY created_at DESC"#,
           &keychain_ids
        )
            .fetch_all(&self.pool)
            .await?;

        let mut utxos = HashMap::new();

        for row in rows {
            let utxo = WalletUtxo {
                wallet_id: row.wallet_id.into(),
                keychain_id: row.keychain_id.into(),
                outpoint: OutPoint {
                    txid: row.tx_id.parse().unwrap(),
                    vout: row.vout as u32,
                },
                kind: KeychainKind::from(row.kind),
                address_idx: row.address_idx as u32,
                value: Satoshis::from(row.value),
                address: row
                    .optional_address
                    .map(|addr| Address::parse_from_trusted_source(&addr)),
                bdk_spent: row.bdk_spent,
                block_height: row.block_height.map(|v| v as u32),
                utxo_detected_ledger_tx_id: LedgerTransactionId::from(
                    row.income_detected_ledger_tx_id,
                ),
                utxo_settled_ledger_tx_id: row
                    .income_settled_ledger_tx_id
                    .map(LedgerTransactionId::from),
                spending_batch_id: row.spending_batch_id.map(BatchId::from),
            };

            let keychain_id = KeychainId::from(row.keychain_id);
            utxos
                .entry(keychain_id)
                .or_insert_with(|| KeychainUtxos {
                    keychain_id,
                    utxos: Vec::new(),
                })
                .utxos
                .push(utxo);
        }

        Ok(utxos)
    }

    pub async fn find_reservable_utxos(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ids: impl Iterator<Item = KeychainId>,
    ) -> Result<Vec<ReservableUtxo>, UtxoError> {
        let uuids = ids.into_iter().map(Uuid::from).collect::<Vec<_>>();
        let rows = sqlx::query!(
            r#"SELECT keychain_id,
               CASE WHEN kind = 'external' THEN true ELSE false END as income_address,
               tx_id, vout, spending_batch_id, income_settled_ledger_tx_id
               FROM bria_utxos
               WHERE keychain_id = ANY($1) AND bdk_spent = false
               FOR UPDATE"#,
            &uuids[..]
        )
        .fetch_all(&mut **tx)
        .await?;

        let reservable_utxos = rows
            .into_iter()
            .map(|row| ReservableUtxo {
                keychain_id: KeychainId::from(row.keychain_id),
                income_address: row.income_address.unwrap_or_default(),
                outpoint: OutPoint {
                    txid: row.tx_id.parse().unwrap(),
                    vout: row.vout as u32,
                },
                spending_batch_id: row.spending_batch_id.map(BatchId::from),
                utxo_settled_ledger_tx_id: row
                    .income_settled_ledger_tx_id
                    .map(LedgerTransactionId::from),
            })
            .collect();

        Ok(reservable_utxos)
    }

    pub async fn reserve_utxos_in_batch(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: AccountId,
        batch_id: BatchId,
        payout_queue_id: PayoutQueueId,
        fee_rate: bitcoin::FeeRate,
        utxos: impl IntoIterator<Item = (KeychainId, OutPoint)>,
    ) -> Result<(), UtxoError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"UPDATE bria_utxos
            SET spending_batch_id = "#,
        );
        query_builder.push_bind(batch_id);
        query_builder.push(", spending_payout_queue_id = ");
        query_builder.push_bind(payout_queue_id);
        query_builder.push(", spending_sats_per_vbyte = ");
        query_builder.push_bind(fee_rate.as_sat_per_vb());
        query_builder.push(" WHERE account_id = ");
        query_builder.push_bind(account_id);
        query_builder.push(" AND (keychain_id, tx_id, vout) IN");
        query_builder.push_tuples(
            utxos
                .into_iter()
                .map(|(keychain_id, utxo)| (keychain_id, utxo.txid.to_string(), utxo.vout as i32)),
            |mut builder, (keychain_id, tx_id, vout)| {
                builder.push_bind(keychain_id);
                builder.push_bind(tx_id);
                builder.push_bind(vout);
            },
        );

        let query = query_builder.build();
        query.execute(&mut **tx).await?;
        Ok(())
    }

    pub async fn accounting_info_for_batch(
        &self,
        batch_id: BatchId,
        wallet_id: WalletId,
    ) -> Result<
        (
            HashMap<LedgerTransactionId, Vec<bitcoin::OutPoint>>,
            Satoshis,
        ),
        UtxoError,
    > {
        let rows = sqlx::query!(
            r#"
          SELECT income_detected_ledger_tx_id, tx_id, vout,
            CASE WHEN income_settled_ledger_tx_id IS NOT NULL THEN value ELSE 0 END AS "value!"
            FROM bria_utxos
            WHERE spending_batch_id = $1 AND wallet_id = $2"#,
            batch_id as BatchId,
            wallet_id as WalletId
        )
        .fetch_all(&self.pool)
        .await?;

        let mut map = HashMap::new();
        let mut sum = Satoshis::ZERO;
        for row in rows {
            map.entry(LedgerTransactionId::from(row.income_detected_ledger_tx_id))
                .or_insert_with(Vec::new)
                .push(bitcoin::OutPoint {
                    txid: row.tx_id.parse().unwrap(),
                    vout: row.vout as u32,
                });
            sum += Satoshis::from(row.value);
        }

        Ok((map, sum))
    }

    pub async fn list_utxos_by_outpoint(
        &self,
        utxos: &HashMap<KeychainId, Vec<OutPoint>>,
    ) -> Result<Vec<WalletUtxo>, UtxoError> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"SELECT wallet_id, keychain_id, tx_id, vout, kind, address_idx, value, address, bdk_spent,
                  CASE
                      WHEN kind = 'external' THEN address
                      ELSE NULL
                  END as optional_address,
                  block_height, income_detected_ledger_tx_id, income_settled_ledger_tx_id, spending_batch_id
            FROM bria_utxos
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
                builder.push_bind(tx_id);
                builder.push_bind(vout);
            },
        );
        query_builder
            .push("ORDER BY block_height ASC NULLS LAST, sats_per_vbyte_when_created DESC");
        let query = query_builder.build();
        let rows = query.fetch_all(&self.pool).await?;

        Ok(rows
            .into_iter()
            .map(|row| WalletUtxo {
                wallet_id: WalletId::from(row.get::<Uuid, _>("wallet_id")),
                keychain_id: KeychainId::from(row.get::<Uuid, _>("keychain_id")),
                address: row
                    .get::<Option<String>, _>("optional_address")
                    .map(|addr| Address::parse_from_trusted_source(&addr)),
                address_idx: row.get::<i32, _>("address_idx") as u32,
                outpoint: OutPoint {
                    txid: row.get::<String, _>("tx_id").parse().unwrap(),
                    vout: row.get::<i32, _>("vout") as u32,
                },
                kind: KeychainKind::from(row.get::<bitcoin::pg::PgKeychainKind, _>("kind")),
                bdk_spent: row.get("bdk_spent"),
                value: Satoshis::from(row.get::<rust_decimal::Decimal, _>("value")),
                utxo_detected_ledger_tx_id: LedgerTransactionId::from(
                    row.get::<Uuid, _>("income_detected_ledger_tx_id"),
                ),
                utxo_settled_ledger_tx_id: row
                    .get::<Option<Uuid>, _>("income_settled_ledger_tx_id")
                    .map(LedgerTransactionId::from),
                spending_batch_id: row
                    .get::<Option<Uuid>, _>("spending_batch_id")
                    .map(BatchId::from),
                block_height: row.get::<Option<i32>, _>("block_height").map(|h| h as u32),
            })
            .collect())
    }

    pub async fn average_utxo_value(
        &self,
        wallet_id: WalletId,
        queue_id: PayoutQueueId,
    ) -> Result<Option<Satoshis>, UtxoError> {
        let row = sqlx::query!(
            r#"SELECT AVG(value) as avg_value
               FROM bria_utxos
               WHERE wallet_id = $1 AND spending_payout_queue_id = $2 AND spending_batch_id IS NOT NULL
        "#,
            wallet_id as WalletId,
            queue_id as PayoutQueueId
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|res| res.avg_value.map(Satoshis::from)))
    }

    pub async fn delete_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        outpoint: bitcoin::OutPoint,
        keychain_id: KeychainId,
    ) -> Result<LedgerTransactionId, UtxoError> {
        let row = sqlx::query!(
            r#"DELETE FROM bria_utxos
            WHERE  keychain_id = $1 AND tx_id = $2 AND vout = $3
            RETURNING income_detected_ledger_tx_id, income_settled_ledger_tx_id"#,
            Uuid::from(keychain_id),
            outpoint.txid.to_string(),
            outpoint.vout as i32,
        )
        .fetch_optional(&mut **tx)
        .await?;
        match row {
            Some(deleted_row) if deleted_row.income_settled_ledger_tx_id.is_none() => Ok(
                LedgerTransactionId::from(deleted_row.income_detected_ledger_tx_id),
            ),
            Some(_) => Err(UtxoError::UtxoAlreadySettledError),
            None => Err(UtxoError::UtxoDoesNotExistError),
        }
    }

    pub async fn find_cpfp_candidates(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ids: impl Iterator<Item = KeychainId>,
        payout_queue_id: PayoutQueueId,
        older_than: chrono::DateTime<chrono::Utc>,
        older_than_block: u32,
    ) -> Result<HashSet<CpfpCandidate>, UtxoError> {
        let keychain_ids: Vec<Uuid> = ids.map(Uuid::from).collect();

        let rows = sqlx::query!(
            r#"
          WITH RECURSIVE unconfirmed_spends AS (
              SELECT * FROM
                  (SELECT
                      u1.keychain_id,
                      u1.tx_id,
                      u1.vout,
                      COALESCE(unnested.ancestor_id, NULL) as ancestor_id,
                      u1.origin_tx_vbytes,
                      u1.origin_tx_fee,
                      TRUE AS utxo_history_tip,
                      u1.origin_tx_batch_id
                  FROM bria_utxos u1
                  LEFT JOIN
                      LATERAL UNNEST(u1.trusted_origin_tx_input_tx_ids) AS unnested(ancestor_id) ON true
                  WHERE
                      u1.origin_tx_payout_queue_id = $1
                      AND u1.keychain_id = ANY($2)
                      AND u1.created_at < $3
                      AND u1.detected_block_height < $4
                      AND bdk_spent IS FALSE
                      AND spend_detected_ledger_tx_id IS NULL
                      AND income_settled_ledger_tx_id IS NULL
                      AND u1.trusted_origin_tx_input_tx_ids IS NOT NULL
                      AND array_length(u1.trusted_origin_tx_input_tx_ids, 1) > 0
                  FOR UPDATE
                  ) AS utxo_history_tips
              UNION ALL
              SELECT
                  u2.keychain_id,
                  u2.tx_id,
                  u2.vout,
                  COALESCE(unnested.ancestor_id, NULL) as ancestor_id,
                  u2.origin_tx_vbytes,
                  u2.origin_tx_fee,
                  FALSE AS utxo_history_tip,
                  u2.origin_tx_batch_id
              FROM bria_utxos u2
              LEFT JOIN
                  LATERAL UNNEST(u2.trusted_origin_tx_input_tx_ids) AS unnested(ancestor_id) ON true
              JOIN
                  unconfirmed_spends ua ON ua.ancestor_id = u2.tx_id
              WHERE 
                  u2.income_settled_ledger_tx_id IS NULL
          )
          SELECT DISTINCT
            keychain_id AS "keychain_id!", tx_id AS "tx_id!", vout AS "vout!", ancestor_id,
            origin_tx_vbytes as "origin_tx_vbytes!", origin_tx_fee as "origin_tx_fee!", utxo_history_tip as "utxo_history_tip!", origin_tx_batch_id
          FROM unconfirmed_spends
          WHERE origin_tx_vbytes IS NOT NULL AND origin_tx_fee IS NOT NULL"#,
            payout_queue_id as PayoutQueueId,
            &keychain_ids,
            older_than,
            older_than_block as i32,
        )
        .fetch_all(&mut **tx)
        .await?;
        let mut candidates = HashSet::new();

        for row in rows {
            let outpoint = OutPoint {
                txid: row.tx_id.parse().expect("couldn't parse txid"),
                vout: row.vout as u32,
            };

            let candidate = CpfpCandidate {
                utxo_history_tip: row.utxo_history_tip,
                keychain_id: KeychainId::from(row.keychain_id),
                outpoint,
                ancestor_tx_id: row
                    .ancestor_id
                    .map(|a_id| a_id.parse().expect("couldn't parse ancestor txid")),
                origin_tx_batch_id: row.origin_tx_batch_id.map(BatchId::from),
                origin_tx_vbytes: row.origin_tx_vbytes as u64,
                origin_tx_fee: Satoshis::from(row.origin_tx_fee as u64),
            };

            candidates.insert(candidate);
        }

        Ok(candidates)
    }
}
