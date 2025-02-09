use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use sqlx_ledger::TransactionId as LedgerTxId;
use tracing::instrument;

use std::collections::HashMap;

use super::{entity::*, error::BatchError};
use crate::primitives::*;

pub struct BatchInfo {
    pub id: BatchId,
    pub payout_queue_id: PayoutQueueId,
    pub created_ledger_tx_id: LedgerTxId,
}

#[derive(Debug, Clone)]
pub struct Batches {
    pool: PgPool,
}

impl Batches {
    pub fn new(pool: &sqlx::PgPool) -> Self {
        Self { pool: pool.clone() }
    }

    #[instrument(name = "batches.create_in_tx", skip_all)]
    pub async fn create_in_tx<'a>(
        &self,
        tx: &mut Transaction<'a, Postgres>,
        batch: NewBatch,
    ) -> Result<BatchId, BatchError> {
        let serializied_psbt = batch.unsigned_psbt.serialize();
        sqlx::query!(
            r#"INSERT INTO bria_batches (id, account_id, payout_queue_id, total_fee_sats, bitcoin_tx_id, unsigned_psbt)
            VALUES ($1, $2, $3, $4, $5, $6)"#,
            batch.id as BatchId,
            batch.account_id as AccountId,
            batch.payout_queue_id as PayoutQueueId,
            i64::from(batch.total_fee_sats),
            batch.tx_id.as_ref() as &[u8],
            serializied_psbt.as_slice() as &[u8],
        ).execute(&mut **tx).await?;

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"INSERT INTO bria_batch_wallet_summaries (
                   batch_id, wallet_id, current_keychain_id, signing_keychains, total_in_sats,
                   total_spent_sats, change_sats, change_address, change_vout, total_fee_sats,
                   cpfp_fee_sats, cpfp_details, batch_created_ledger_tx_id, batch_broadcast_ledger_tx_id, batch_cancel_ledger_tx_id
               )"#,
        );
        query_builder.push_values(
            batch.wallet_summaries,
            |mut builder, (wallet_id, summary)| {
                builder.push_bind(batch.id);
                builder.push_bind(wallet_id);
                builder.push_bind(summary.current_keychain_id);
                builder.push_bind(
                    summary
                        .signing_keychains
                        .into_iter()
                        .map(uuid::Uuid::from)
                        .collect::<Vec<_>>(),
                );
                builder.push_bind(i64::from(summary.total_in_sats));
                builder.push_bind(i64::from(summary.total_spent_sats));
                builder.push_bind(i64::from(summary.change_sats));
                builder.push_bind(summary.change_address.map(|a| a.to_string()));
                builder.push_bind(summary.change_outpoint.map(|out| out.vout as i32));
                builder.push_bind(i64::from(summary.total_fee_sats));
                builder.push_bind(i64::from(summary.cpfp_fee_sats));
                builder.push_bind(serde_json::to_value(summary.cpfp_details).unwrap());
                builder.push_bind(summary.batch_created_ledger_tx_id);
                builder.push_bind(summary.batch_broadcast_ledger_tx_id);
                builder.push_bind(summary.batch_cancel_ledger_tx_id);
            },
        );
        let query = query_builder.build();
        query.execute(&mut **tx).await?;

        Ok(batch.id)
    }

    #[instrument(name = "batches.find_by_id", skip_all)]
    pub async fn find_by_id(
        &self,
        account_id: AccountId,
        id: BatchId,
    ) -> Result<Batch, BatchError> {
        let rows = sqlx::query!(
            r#"SELECT
                    payout_queue_id, unsigned_psbt, signed_tx, bitcoin_tx_id, s.batch_id,
                    s.wallet_id, s.current_keychain_id, s.signing_keychains, total_in_sats,
                    total_spent_sats, change_sats, change_address, change_vout, s.total_fee_sats,
                    cpfp_fee_sats, cpfp_details, batch_created_ledger_tx_id, batch_broadcast_ledger_tx_id, batch_cancel_ledger_tx_id
            FROM bria_batch_wallet_summaries s
            LEFT JOIN bria_batches b ON b.id = s.batch_id
            WHERE s.batch_id = $1 AND b.account_id = $2"#,
            id as BatchId,
            account_id as AccountId
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Err(BatchError::BatchIdNotFound(id.to_string()));
        }

        let mut wallet_summaries = HashMap::new();
        let bitcoin_tx_id = bitcoin::consensus::deserialize(&rows[0].bitcoin_tx_id)?;
        let unsigned_psbt =
            bitcoin::psbt::PartiallySignedTransaction::deserialize(&rows[0].unsigned_psbt)?;
        let signed_tx = rows[0]
            .signed_tx
            .as_ref()
            .map(|tx| bitcoin::consensus::deserialize(tx))
            .transpose()?;
        let payout_queue_id = PayoutQueueId::from(rows[0].payout_queue_id);

        for row in rows.into_iter() {
            let wallet_id = WalletId::from(row.wallet_id);
            wallet_summaries.insert(
                wallet_id,
                WalletSummary {
                    wallet_id,
                    signing_keychains: row
                        .signing_keychains
                        .iter()
                        .map(|k| KeychainId::from(*k))
                        .collect(),
                    total_in_sats: Satoshis::from(row.total_in_sats),
                    total_spent_sats: Satoshis::from(row.total_spent_sats),
                    total_fee_sats: Satoshis::from(row.total_fee_sats),
                    cpfp_fee_sats: Satoshis::from(row.cpfp_fee_sats),
                    cpfp_details: serde_json::from_value(row.cpfp_details)
                        .expect("parse cpfp details"),
                    change_sats: Satoshis::from(row.change_sats),
                    change_address: row
                        .change_address
                        .as_ref()
                        .map(|a| Address::parse_from_trusted_source(a)),
                    change_outpoint: row.change_vout.map(|out| bitcoin::OutPoint {
                        txid: bitcoin_tx_id,
                        vout: out as u32,
                    }),
                    current_keychain_id: KeychainId::from(row.current_keychain_id),
                    batch_created_ledger_tx_id: row
                        .batch_created_ledger_tx_id
                        .map(LedgerTxId::from),
                    batch_broadcast_ledger_tx_id: row
                        .batch_broadcast_ledger_tx_id
                        .map(LedgerTxId::from),
                    batch_cancel_ledger_tx_id: row.batch_cancel_ledger_tx_id.map(LedgerTxId::from),
                },
            );
        }

        Ok(Batch {
            id,
            account_id,
            payout_queue_id,
            bitcoin_tx_id,
            unsigned_psbt,
            signed_tx,
            wallet_summaries,
        })
    }

    #[instrument(name = "batches.set_signed_tx", skip(self))]
    pub async fn set_signed_tx(
        &self,
        batch_id: BatchId,
        bitcoin_tx: bitcoin::Transaction,
    ) -> Result<(), BatchError> {
        sqlx::query!(
            r#"UPDATE bria_batches SET signed_tx = $1 WHERE id = $2"#,
            bitcoin::consensus::encode::serialize(&bitcoin_tx),
            batch_id as BatchId,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[instrument(name = "batches.set_batch_created_ledger_tx_id", skip(self))]
    pub async fn set_batch_created_ledger_tx_id(
        &self,
        batch_id: BatchId,
        wallet_id: WalletId,
    ) -> Result<Option<(Transaction<'_, Postgres>, LedgerTxId)>, BatchError> {
        let mut tx = self.pool.begin().await?;
        let ledger_transaction_id = LedgerTxId::new();
        let rows_affected = sqlx::query!(
            r#"UPDATE bria_batch_wallet_summaries
               SET batch_created_ledger_tx_id = $1
               WHERE wallet_id = $2 AND batch_id = $3 AND batch_created_ledger_tx_id IS NULL"#,
            ledger_transaction_id as LedgerTxId,
            wallet_id as WalletId,
            batch_id as BatchId,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows_affected > 0 {
            Ok(Some((tx, ledger_transaction_id)))
        } else {
            Ok(None)
        }
    }

    #[instrument(name = "batches.set_batch_broadcast_ledger_tx_id", skip(self))]
    pub async fn set_batch_broadcast_ledger_tx_id(
        &self,
        bitcoin_tx_id: bitcoin::Txid,
        wallet_id: WalletId,
    ) -> Result<Option<(Transaction<'_, Postgres>, BatchInfo, LedgerTxId)>, BatchError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query!(
            r#"WITH b AS (
                 SELECT id, payout_queue_id FROM bria_batches
                 WHERE bitcoin_tx_id = $1
               )
               SELECT b.id, b. payout_queue_id, s.batch_broadcast_ledger_tx_id as "ledger_id?", s.batch_created_ledger_tx_id
               FROM b
               LEFT JOIN (
                   SELECT batch_id, batch_broadcast_ledger_tx_id, batch_created_ledger_tx_id
                   FROM bria_batch_wallet_summaries
                   WHERE wallet_id = $2 AND batch_id = ANY(SELECT id FROM b)
                   FOR UPDATE
               ) s
               ON b.id = s.batch_id"#,
            bitcoin_tx_id.as_ref() as &[u8],
            wallet_id as WalletId
        )
        .fetch_optional(&mut *tx)
        .await?;
        if row.is_none() || row.as_ref().unwrap().batch_created_ledger_tx_id.is_none() {
            return Ok(None);
        }
        let row = row.unwrap();
        let created_ledger_tx_id = LedgerTxId::from(row.batch_created_ledger_tx_id.unwrap());
        let batch_id = BatchId::from(row.id);
        let payout_queue_id = PayoutQueueId::from(row.payout_queue_id);
        if row.ledger_id.is_some() {
            return Ok(Some((
                tx,
                BatchInfo {
                    id: batch_id,
                    payout_queue_id,
                    created_ledger_tx_id,
                },
                LedgerTxId::from(row.ledger_id.unwrap()),
            )));
        }
        let ledger_transaction_id = LedgerTxId::new();
        sqlx::query!(
            r#"UPDATE bria_batch_wallet_summaries
               SET batch_broadcast_ledger_tx_id = $1
               WHERE bria_batch_wallet_summaries.batch_id = $2
                 AND bria_batch_wallet_summaries.wallet_id = $3"#,
            ledger_transaction_id as LedgerTxId,
            batch_id as BatchId,
            wallet_id as WalletId,
        )
        .execute(&mut *tx)
        .await?;

        Ok(Some((
            tx,
            BatchInfo {
                id: batch_id,
                payout_queue_id,
                created_ledger_tx_id,
            },
            ledger_transaction_id,
        )))
    }

    #[instrument(name = "batches.set_batch_cancel_ledger_tx_id", skip(self))]
    pub async fn set_batch_cancel_ledger_tx_id(
        &self,
        batch_id: BatchId,
    ) -> Result<Option<(Transaction<'_, Postgres>, BatchInfo, LedgerTxId)>, BatchError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query!(
            r#"SELECT
                bb.id,
                bb.payout_queue_id,
                bbws.batch_cancel_ledger_tx_id as "ledger_id?",
                bbws.batch_created_ledger_tx_id
            FROM bria_batches bb
            INNER JOIN bria_batch_wallet_summaries bbws ON bb.id = bbws.batch_id
            WHERE bb.id = $1
            FOR UPDATE"#,
            batch_id as BatchId,
        )
        .fetch_optional(&mut *tx)
        .await?;
        if row.is_none() || row.as_ref().unwrap().batch_created_ledger_tx_id.is_none() {
            return Ok(None);
        }
        let row = row.unwrap();
        let created_ledger_tx_id = LedgerTxId::from(row.batch_created_ledger_tx_id.unwrap());
        let batch_id = BatchId::from(row.id);
        let payout_queue_id = PayoutQueueId::from(row.payout_queue_id);
        if row.ledger_id.is_some() {
            return Ok(Some((
                tx,
                BatchInfo {
                    id: batch_id,
                    payout_queue_id,
                    created_ledger_tx_id,
                },
                LedgerTxId::from(row.ledger_id.unwrap()),
            )));
        }
        let ledger_transaction_id = LedgerTxId::new();
        sqlx::query!(
            r#"UPDATE bria_batch_wallet_summaries
               SET batch_cancel_ledger_tx_id = $1
               WHERE bria_batch_wallet_summaries.batch_id = $2"#,
            ledger_transaction_id as LedgerTxId,
            batch_id as BatchId,
        )
        .execute(&mut *tx)
        .await?;

        Ok(Some((
            tx,
            BatchInfo {
                id: batch_id,
                payout_queue_id,
                created_ledger_tx_id,
            },
            ledger_transaction_id,
        )))
    }
}
