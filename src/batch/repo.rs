use std::{collections::HashMap, str, str::FromStr};

use bitcoin::{consensus::encode, Address, Txid};
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use sqlx_ledger::TransactionId;
use tracing::instrument;
use uuid::Uuid;

use super::entity::*;
use crate::{error::*, primitives::*};

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
            batch.total_fee_sats as i64,
            batch.tx_id.as_ref(),
            encode::serialize(&batch.unsigned_psbt)
        ).execute(&mut *tx).await?;

        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"INSERT INTO bria_batch_wallet_summaries
            (batch_id, wallet_id, total_in_sats, total_out_sats, change_sats, change_address, fee_sats)"#,
        );
        query_builder.push_values(
            batch.wallet_summaries,
            |mut builder, (wallet_id, summary)| {
                builder.push_bind(Uuid::from(batch.id));
                builder.push_bind(Uuid::from(wallet_id));
                builder.push_bind(summary.total_in_sats as i64);
                builder.push_bind(summary.total_out_sats as i64);
                builder.push_bind(summary.change_sats as i64);
                builder.push_bind(summary.change_address.to_string());
                builder.push_bind(summary.fee_sats as i64);
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

        let utxos = batch
            .included_utxos
            .into_iter()
            .flat_map(|(keychain_id, utxos)| {
                utxos.into_iter().map(move |utxo| (keychain_id, utxo))
            });
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"INSERT INTO bria_batch_utxos
            (batch_id, keychain_id, tx_id, vout)"#,
        );
        query_builder.push_values(utxos, |mut builder, (keychain_id, utxo)| {
            builder.push_bind(Uuid::from(batch.id));
            builder.push_bind(Uuid::from(keychain_id));
            builder.push_bind(utxo.txid.to_string());
            builder.push_bind(utxo.vout as i32);
        });
        let query = query_builder.build();
        query.execute(&mut *tx).await?;

        Ok(batch.id)
    }

    #[instrument(name = "batches.find_by_id", skip_all)]
    pub async fn find_by_id<'a>(&self, id: BatchId) -> Result<Batch, BriaError> {
        let rows = sqlx::query!(
            r#"SELECT batch_group_id, bitcoin_tx_id, batch_id, wallet_id, total_in_sats, total_out_sats, change_sats, change_address, fee_sats, ledger_tx_pending_id, ledger_tx_settled_id
            FROM bria_batch_wallet_summaries
            LEFT JOIN bria_batches ON id = batch_id
            WHERE batch_id = $1"#,
            Uuid::from(id)
        ).fetch_all(&self.pool).await?;

        if rows.len() == 0 {
            return Err(BriaError::BatchNotFound);
        }

        let mut wallet_summaries = HashMap::new();
        for row in rows.iter() {
            let wallet_id = WalletId::from(row.wallet_id);
            wallet_summaries.insert(
                wallet_id,
                WalletSummary {
                    wallet_id,
                    total_in_sats: u64::try_from(row.total_in_sats)?,
                    total_out_sats: u64::try_from(row.total_out_sats)?,
                    fee_sats: u64::try_from(row.fee_sats)?,
                    change_sats: u64::try_from(row.change_sats)?,
                    change_address: Address::from_str(&row.change_address)?,
                    ledger_tx_pending_id: TransactionId::from(row.ledger_tx_pending_id),
                    ledger_tx_settled_id: TransactionId::from(row.ledger_tx_settled_id),
                },
            );
        }

        Ok(Batch {
            id,
            batch_group_id: BatchGroupId::from(rows[0].batch_group_id),
            bitcoin_tx_id: Txid::from_str(
                str::from_utf8(&rows[0].bitcoin_tx_id)
                    .expect("Couldn't convert bitcoin tx id to string"),
            )
            .expect("Couldn't parse bitcoin tx id"),
            wallet_summaries,
        })
    }
}
