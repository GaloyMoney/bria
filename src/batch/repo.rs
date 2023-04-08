use std::{collections::HashMap, str::FromStr};

use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use sqlx_ledger::TransactionId as LedgerTxId;
use tracing::instrument;
use uuid::Uuid;

use super::entity::*;
use crate::{
    error::*,
    primitives::{bitcoin::*, *},
};

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
    ) -> Result<BatchId, BriaError> {
        sqlx::query!(
            r#"INSERT INTO bria_batches (id, batch_group_id, total_fee_sats, bitcoin_tx_id, unsigned_psbt)
            VALUES ($1, $2, $3, $4, $5)"#,
            Uuid::from(batch.id),
            Uuid::from(batch.batch_group_id),
            i64::from(batch.total_fee_sats),
            batch.tx_id.as_ref(),
            bitcoin::consensus::encode::serialize(&batch.unsigned_psbt)
        ).execute(&mut *tx).await?;

        let utxos = batch.iter_utxos();
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"INSERT INTO bria_batch_spent_utxos
            (batch_id, wallet_id, keychain_id, tx_id, vout)"#,
        );
        query_builder.push_values(utxos, |mut builder, (wallet_id, keychain_id, utxo)| {
            builder.push_bind(Uuid::from(batch.id));
            builder.push_bind(Uuid::from(wallet_id));
            builder.push_bind(Uuid::from(keychain_id));
            builder.push_bind(utxo.txid.to_string());
            builder.push_bind(utxo.vout as i32);
        });
        let query = query_builder.build();
        query.execute(&mut *tx).await?;

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"INSERT INTO bria_batch_wallet_summaries
            (batch_id, wallet_id, total_in_sats, total_spent_sats, change_sats, change_address, change_vout, change_keychain_id, fee_sats, create_batch_ledger_tx_id, submitted_ledger_tx_id)"#,
        );
        query_builder.push_values(
            batch.wallet_summaries,
            |mut builder, (wallet_id, summary)| {
                builder.push_bind(Uuid::from(batch.id));
                builder.push_bind(Uuid::from(wallet_id));
                builder.push_bind(i64::from(summary.total_in_sats));
                builder.push_bind(i64::from(summary.total_spent_sats));
                builder.push_bind(i64::from(summary.change_sats));
                builder.push_bind(summary.change_address.to_string());
                builder.push_bind(summary.change_outpoint.map(|out| out.vout as i32));
                builder.push_bind(Uuid::from(summary.change_keychain_id));
                builder.push_bind(i64::from(summary.fee_sats));
                builder.push_bind(summary.create_batch_ledger_tx_id.map(Uuid::from));
                builder.push_bind(summary.submitted_ledger_tx_id.map(Uuid::from));
            },
        );
        let query = query_builder.build();
        query.execute(&mut *tx).await?;

        let payout_ids = batch.included_payouts.into_values().flatten();
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"INSERT INTO bria_batch_payouts
            (batch_id, payout_id)"#,
        );
        query_builder.push_values(payout_ids, |mut builder, id| {
            builder.push_bind(Uuid::from(batch.id));
            builder.push_bind(Uuid::from(id));
        });
        let query = query_builder.build();
        query.execute(&mut *tx).await?;

        Ok(batch.id)
    }

    #[instrument(name = "batches.find_by_id", skip_all)]
    pub async fn find_by_id(&self, id: BatchId) -> Result<Batch, BriaError> {
        let rows = sqlx::query!(
            r#"SELECT batch_group_id, unsigned_psbt, bitcoin_tx_id, u.batch_id, s.wallet_id, total_in_sats, total_spent_sats, change_sats, change_address, change_vout, change_keychain_id, fee_sats, create_batch_ledger_tx_id, submitted_ledger_tx_id, tx_id, vout, keychain_id
            FROM bria_batch_spent_utxos u
            LEFT JOIN bria_batch_wallet_summaries s ON u.batch_id = s.batch_id AND u.wallet_id = s.wallet_id
            LEFT JOIN bria_batches b ON b.id = u.batch_id
            WHERE u.batch_id = $1"#,
            Uuid::from(id)
        ).fetch_all(&self.pool).await?;

        if rows.is_empty() {
            return Err(BriaError::BatchNotFound);
        }

        let mut wallet_summaries = HashMap::new();
        let mut included_utxos: HashMap<WalletId, HashMap<KeychainId, Vec<OutPoint>>> =
            HashMap::new();
        let bitcoin_tx_id = bitcoin::consensus::deserialize(&rows[0].bitcoin_tx_id)?;
        let unsigned_psbt = bitcoin::consensus::deserialize(&rows[0].unsigned_psbt)?;
        for row in rows.iter() {
            let wallet_id = WalletId::from(row.wallet_id);
            let keychain_id = KeychainId::from(row.keychain_id);
            included_utxos
                .entry(wallet_id)
                .or_default()
                .entry(keychain_id)
                .or_default()
                .push(OutPoint {
                    txid: row.tx_id.parse().expect("invalid txid"),
                    vout: row.vout as u32,
                });
            wallet_summaries.insert(
                wallet_id,
                WalletSummary {
                    wallet_id,
                    total_in_sats: Satoshis::from(row.total_in_sats),
                    total_spent_sats: Satoshis::from(row.total_spent_sats),
                    fee_sats: Satoshis::from(row.fee_sats),
                    change_sats: Satoshis::from(row.change_sats),
                    change_address: Address::from_str(&row.change_address)?,
                    change_outpoint: row.change_vout.map(|out| bitcoin::OutPoint {
                        txid: bitcoin_tx_id,
                        vout: out as u32,
                    }),
                    change_keychain_id: KeychainId::from(row.change_keychain_id),
                    create_batch_ledger_tx_id: row.create_batch_ledger_tx_id.map(LedgerTxId::from),
                    submitted_ledger_tx_id: row.submitted_ledger_tx_id.map(LedgerTxId::from),
                },
            );
        }

        Ok(Batch {
            id,
            batch_group_id: BatchGroupId::from(rows[0].batch_group_id),
            bitcoin_tx_id,
            unsigned_psbt,
            wallet_summaries,
            included_utxos,
        })
    }

    #[instrument(name = "batches.find_containing_utxo", skip(self))]
    pub async fn find_containing_utxo(
        &self,
        keychain_id: KeychainId,
        utxo: OutPoint,
    ) -> Result<Option<BatchId>, BriaError> {
        let row = sqlx::query!(
            r#"SELECT batch_id
            FROM bria_batch_spent_utxos
            WHERE keychain_id = $1 AND tx_id = $2 AND vout = $3"#,
            Uuid::from(keychain_id),
            utxo.txid.to_string(),
            utxo.vout as i32
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| BatchId::from(row.batch_id)))
    }

    #[instrument(name = "batches.set_create_batch_ledger_tx_id", skip(self))]
    pub async fn set_create_batch_ledger_tx_id(
        &self,
        batch_id: BatchId,
        wallet_id: WalletId,
    ) -> Result<Option<(Transaction<'_, Postgres>, LedgerTxId)>, BriaError> {
        let mut tx = self.pool.begin().await?;
        let ledger_transaction_id = Uuid::new_v4();
        let rows_affected = sqlx::query!(
            r#"UPDATE bria_batch_wallet_summaries
               SET create_batch_ledger_tx_id = $1
               WHERE wallet_id = $2 AND batch_id = $3 AND create_batch_ledger_tx_id IS NULL"#,
            ledger_transaction_id,
            Uuid::from(wallet_id),
            Uuid::from(batch_id),
        )
        .execute(&mut tx)
        .await?
        .rows_affected();

        if rows_affected > 0 {
            Ok(Some((tx, LedgerTxId::from(ledger_transaction_id))))
        } else {
            Ok(None)
        }
    }
}
