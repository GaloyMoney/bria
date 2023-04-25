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
            r#"INSERT INTO bria_batches (id, account_id, batch_group_id, total_fee_sats, bitcoin_tx_id, unsigned_psbt)
            VALUES ($1, $2, $3, $4, $5, $6)"#,
            Uuid::from(batch.id),
            Uuid::from(batch.account_id),
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

        Ok(batch.id)
    }

    #[instrument(name = "batches.find_by_id", skip_all)]
    pub async fn find_by_id(&self, account_id: AccountId, id: BatchId) -> Result<Batch, BriaError> {
        let rows = sqlx::query!(
            r#"SELECT batch_group_id, unsigned_psbt, signed_tx, bitcoin_tx_id, u.batch_id, s.wallet_id, total_in_sats, total_spent_sats, change_sats, change_address, change_vout, change_keychain_id, fee_sats, create_batch_ledger_tx_id, submitted_ledger_tx_id, tx_id, vout, keychain_id
            FROM bria_batch_spent_utxos u
            LEFT JOIN bria_batch_wallet_summaries s ON u.batch_id = s.batch_id AND u.wallet_id = s.wallet_id
            LEFT JOIN bria_batches b ON b.id = u.batch_id
            WHERE u.batch_id = $1 AND b.account_id = $2"#,
            Uuid::from(id),
            Uuid::from(account_id)
        ).fetch_all(&self.pool).await?;

        if rows.is_empty() {
            return Err(BriaError::BatchNotFound);
        }

        let mut wallet_summaries = HashMap::new();
        let mut included_utxos: HashMap<WalletId, HashMap<KeychainId, Vec<OutPoint>>> =
            HashMap::new();
        let bitcoin_tx_id = bitcoin::consensus::deserialize(&rows[0].bitcoin_tx_id)?;
        let unsigned_psbt = bitcoin::consensus::deserialize(&rows[0].unsigned_psbt)?;
        let signed_tx = rows[0]
            .signed_tx
            .as_ref()
            .map(|tx| bitcoin::consensus::deserialize(tx))
            .transpose()?;
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
            account_id,
            batch_group_id: BatchGroupId::from(rows[0].batch_group_id),
            bitcoin_tx_id,
            unsigned_psbt,
            signed_tx,
            wallet_summaries,
            included_utxos,
        })
    }

    #[instrument(name = "batches.set_signed_tx", skip(self))]
    pub async fn set_signed_tx(
        &self,
        batch_id: BatchId,
        bitcoin_tx: bitcoin::Transaction,
    ) -> Result<(), BriaError> {
        sqlx::query!(
            r#"UPDATE bria_batches SET signed_tx = $1 WHERE id = $2"#,
            bitcoin::consensus::encode::serialize(&bitcoin_tx),
            Uuid::from(batch_id),
        )
        .execute(&self.pool)
        .await?;

        Ok(())
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

    #[instrument(name = "batches.set_submitted_ledger_tx_id", skip(self))]
    pub async fn set_submitted_ledger_tx_id(
        &self,
        bitcoin_tx_id: bitcoin::Txid,
        wallet_id: WalletId,
    ) -> Result<Option<(Transaction<'_, Postgres>, LedgerTxId, LedgerTxId)>, BriaError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query!(
            r#"WITH b AS (
                 SELECT id FROM bria_batches
                 WHERE bitcoin_tx_id = $1
               )
               SELECT b.id, s.submitted_ledger_tx_id as "ledger_id?", s.create_batch_ledger_tx_id
               FROM b
               LEFT JOIN (
                   SELECT batch_id, submitted_ledger_tx_id, create_batch_ledger_tx_id
                   FROM bria_batch_wallet_summaries
                   WHERE wallet_id = $2 AND batch_id = ANY(SELECT id FROM b)
                   FOR UPDATE
               ) s
               ON b.id = s.batch_id"#,
            bitcoin_tx_id.as_ref(),
            Uuid::from(wallet_id)
        )
        .fetch_optional(&mut tx)
        .await?;
        if row.is_none() || row.as_ref().unwrap().create_batch_ledger_tx_id.is_none() {
            return Ok(None);
        }
        let row = row.unwrap();
        let create_batch_ledger_tx_id = LedgerTxId::from(row.create_batch_ledger_tx_id.unwrap());
        let batch_id = row.id;
        if row.ledger_id.is_some() {
            return Ok(Some((
                tx,
                create_batch_ledger_tx_id,
                LedgerTxId::from(row.ledger_id.unwrap()),
            )));
        }
        let ledger_transaction_id = Uuid::new_v4();
        sqlx::query!(
            r#"UPDATE bria_batch_wallet_summaries
               SET submitted_ledger_tx_id = $1
               WHERE bria_batch_wallet_summaries.batch_id = $2
                 AND bria_batch_wallet_summaries.wallet_id = $3"#,
            ledger_transaction_id,
            Uuid::from(batch_id),
            Uuid::from(wallet_id),
        )
        .execute(&mut tx)
        .await?;

        Ok(Some((
            tx,
            create_batch_ledger_tx_id,
            LedgerTxId::from(ledger_transaction_id),
        )))
    }
}
