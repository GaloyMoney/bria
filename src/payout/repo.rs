use sqlx::{Pool, Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

use super::entity::*;
use crate::{error::*, primitives::*};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Payouts {
    pool: Pool<Postgres>,
}

impl Payouts {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    #[instrument(name = "payouts.create", skip(self, tx))]
    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: AccountId,
        new_payout: NewPayout,
    ) -> Result<PayoutId, BriaError> {
        let NewPayout {
            id,
            batch_group_id,
            wallet_id,
            satoshis,
            destination,
            external_id,
            metadata,
        } = new_payout;
        sqlx::query!(
            r#"INSERT INTO bria_payouts (id, account_id, batch_group_id, wallet_id, satoshis, destination_data, external_id, metadata)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
            Uuid::from(id),
            Uuid::from(account_id),
            Uuid::from(batch_group_id),
            Uuid::from(wallet_id),
            i64::from(satoshis),
            serde_json::to_value(destination)?,
            external_id,
            metadata
        ).execute(&mut *tx).await?;
        Ok(id)
    }

    #[instrument(name = "payouts.list_unbatched", skip(self))]
    pub async fn list_unbatched(
        &self,
        batch_group_id: BatchGroupId,
    ) -> Result<HashMap<WalletId, Vec<UnbatchedPayout>>, BriaError> {
        let rows = sqlx::query!(
            r#"WITH latest AS (
                 SELECT DISTINCT(id), MAX(version) OVER (PARTITION BY id ORDER BY version DESC)
                 FROM bria_payouts LEFT JOIN bria_batch_payouts ON id = payout_id
                 WHERE batch_group_id = $1 AND payout_id IS NULL
               ) SELECT id, wallet_id, destination_data, satoshis FROM bria_payouts
                 WHERE (id, version) IN (SELECT * FROM latest)
                 ORDER BY priority, created_at"#,
            Uuid::from(batch_group_id),
        )
        .fetch_all(&self.pool)
        .await?;
        let mut payouts = HashMap::new();
        for row in rows {
            let wallet_id = WalletId::from(row.wallet_id);
            let payouts = payouts.entry(wallet_id).or_insert_with(Vec::new);
            payouts.push(UnbatchedPayout {
                id: PayoutId::from(row.id),
                wallet_id,
                destination: serde_json::from_value(row.destination_data)
                    .expect("Couldn't deserialize destination"),
                satoshis: Satoshis::from(row.satoshis),
            });
        }
        Ok(payouts)
    }

    #[instrument(name = "payouts.list_for_wallet", skip(self))]
    pub async fn list_for_wallet(&self, wallet_id: WalletId) -> Result<Vec<Payout>, BriaError> {
        let rows = sqlx::query!(
        r#"WITH latest AS (
             SELECT DISTINCT(id), MAX(version) OVER (PARTITION BY id ORDER BY version DESC)
             FROM bria_payouts
             WHERE wallet_id = $1
           ) SELECT bria_payouts.id, batch_group_id, bria_batch_payouts.batch_id, satoshis, destination_data, external_id, metadata FROM bria_payouts
             LEFT JOIN bria_batch_payouts ON bria_payouts.id = bria_batch_payouts.payout_id
             WHERE (bria_payouts.id, version) IN (SELECT * FROM latest)
             ORDER BY created_at"#,
        Uuid::from(wallet_id),
    )
    .fetch_all(&self.pool)
    .await?;

        let payouts = rows
            .into_iter()
            .map(|row| Payout {
                id: PayoutId::from(row.id),
                wallet_id,
                batch_group_id: BatchGroupId::from(row.batch_group_id),
                batch_id: Some(BatchId::from(row.batch_id)),
                satoshis: Satoshis::from(row.satoshis),
                destination: serde_json::from_value(row.destination_data)
                    .expect("Couldn't deserialize destination"),
                external_id: row.external_id,
                metadata: row.metadata,
            })
            .collect();

        Ok(payouts)
    }
}
