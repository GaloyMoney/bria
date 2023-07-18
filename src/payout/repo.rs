use sqlx::{Pool, Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

use std::collections::HashMap;

use super::{entity::*, error::*, unbatched::*};
use crate::{entity::*, primitives::*};

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
        new_payout: NewPayout,
    ) -> Result<PayoutId, PayoutError> {
        sqlx::query!(
            r#"INSERT INTO bria_payouts (id, account_id, wallet_id, payout_queue_id, profile_id, external_id)
               VALUES ($1, $2, $3, $4, $5, $6)"#,
            Uuid::from(new_payout.id),
            Uuid::from(new_payout.account_id),
            Uuid::from(new_payout.wallet_id),
            Uuid::from(new_payout.payout_queue_id),
            Uuid::from(new_payout.profile_id),
            new_payout.external_id,
        ).execute(&mut **tx).await?;
        let id = new_payout.id;
        EntityEvents::<PayoutEvent>::persist(
            "bria_payout_events",
            tx,
            new_payout.initial_events().new_serialized_events(id),
        )
        .await?;
        Ok(id)
    }

    #[instrument(name = "payouts.find_by_id", skip(self))]
    pub async fn find_by_id(
        &self,
        account_id: AccountId,
        payout_id: PayoutId,
    ) -> Result<Payout, PayoutError> {
        let rows = sqlx::query!(
            r#"
          SELECT b.*, e.sequence, e.event
          FROM bria_payouts b
          JOIN bria_payout_events e ON b.id = e.id
          WHERE account_id = $1 AND b.id = $2
          ORDER BY b.created_at, b.id, e.sequence"#,
            account_id as AccountId,
            payout_id as PayoutId,
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Err(PayoutError::PayoutIdNotFound(payout_id.to_string()));
        }

        let mut entity_events = EntityEvents::new();
        for row in rows {
            entity_events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(Payout::try_from(entity_events)?)
    }

    #[instrument(name = "payouts.find_by_external_id", skip(self))]
    pub async fn find_by_external_id(
        &self,
        account_id: AccountId,
        external_id: String,
    ) -> Result<Payout, PayoutError> {
        let rows = sqlx::query!(
            r#"
          SELECT b.*, e.sequence, e.event
          FROM bria_payouts b
          JOIN bria_payout_events e ON b.id = e.id
          WHERE account_id = $1 AND b.external_id = $2
          ORDER BY b.created_at, b.id, e.sequence"#,
            account_id as AccountId,
            external_id
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Err(PayoutError::ExternalIdNotFound);
        }

        let mut entity_events = EntityEvents::new();
        for row in rows {
            entity_events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(Payout::try_from(entity_events)?)
    }

    #[instrument(name = "payouts.list_unbatched", skip(self))]
    pub async fn list_unbatched(
        &self,
        account_id: AccountId,
        payout_queue_id: PayoutQueueId,
    ) -> Result<UnbatchedPayouts, PayoutError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_payouts b
              JOIN bria_payout_events e ON b.id = e.id
              WHERE b.batch_id IS NULL AND b.account_id = $1 AND b.payout_queue_id = $2
              ORDER BY b.created_at, b.id, e.sequence FOR UPDATE"#,
            account_id as AccountId,
            payout_queue_id as PayoutQueueId,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut wallet_payouts = Vec::new();
        let mut entity_events = HashMap::new();
        for row in rows {
            let wallet_id = WalletId::from(row.wallet_id);
            let id = WalletId::from(row.id);
            wallet_payouts.push((id, wallet_id));
            let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(row.sequence as usize, row.event)?;
        }
        let mut payouts: HashMap<WalletId, Vec<UnbatchedPayout>> = HashMap::new();
        for (id, wallet_id) in wallet_payouts {
            if let Some(events) = entity_events.remove(&id) {
                payouts
                    .entry(wallet_id)
                    .or_default()
                    .push(UnbatchedPayout::try_from(events)?);
            }
        }
        let filtered_payouts: HashMap<WalletId, Vec<UnbatchedPayout>> = payouts
            .into_iter()
            .map(|(wallet_id, unbatched_payouts)| {
                let filtered_unbatched_payouts = unbatched_payouts
                    .into_iter()
                    .filter(|payout| {
                        !payout
                            .events
                            .iter()
                            .any(|event| matches!(event, PayoutEvent::Cancelled { .. }))
                    })
                    .collect();
                (wallet_id, filtered_unbatched_payouts)
            })
            .collect();
        Ok(UnbatchedPayouts::new(filtered_payouts))
    }

    #[instrument(name = "payouts.list_for_wallet", skip(self))]
    pub async fn list_for_wallet(
        &self,
        account_id: AccountId,
        wallet_id: WalletId,
    ) -> Result<Vec<Payout>, PayoutError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_payouts b
              JOIN bria_payout_events e ON b.id = e.id
              WHERE b.account_id = $1 AND b.wallet_id = $2
              ORDER BY b.created_at, b.id, e.sequence FOR UPDATE"#,
            Uuid::from(account_id),
            Uuid::from(wallet_id)
        )
        .fetch_all(&self.pool)
        .await?;
        let mut wallet_payouts = Vec::new();
        let mut entity_events = HashMap::new();
        for row in rows {
            let id = WalletId::from(row.id);
            wallet_payouts.push(id);
            let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(row.sequence as usize, row.event)?;
        }
        let mut payouts = Vec::new();
        for id in wallet_payouts {
            if let Some(events) = entity_events.remove(&id) {
                payouts.push(Payout::try_from(events)?);
            }
        }
        Ok(payouts)
    }

    #[instrument(name = "payouts.list_for_batch", skip(self))]
    pub async fn list_for_batch(
        &self,
        account_id: AccountId,
        batch_id: BatchId,
    ) -> Result<HashMap<WalletId, Vec<Payout>>, PayoutError> {
        let rows = sqlx::query!(
            r#"
              SELECT b.*, e.sequence, e.event
              FROM bria_payouts b
              JOIN bria_payout_events e ON b.id = e.id
              WHERE b.account_id = $1 AND b.batch_id = $2
              ORDER BY b.created_at, b.id, e.sequence"#,
            account_id as AccountId,
            batch_id as BatchId,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut payout_ids = Vec::new();
        let mut entity_events = HashMap::new();
        for row in rows {
            let id = PayoutId::from(row.id);
            payout_ids.push(id);
            let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(row.sequence as usize, row.event)?;
        }
        let mut payouts: HashMap<WalletId, Vec<Payout>> = HashMap::new();
        for id in payout_ids {
            if let Some(events) = entity_events.remove(&id) {
                let payout = Payout::try_from(events)?;
                payouts.entry(payout.wallet_id).or_default().push(payout);
            }
        }
        Ok(payouts)
    }

    pub async fn update_unbatched(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        payouts: UnbatchedPayouts,
    ) -> Result<(), PayoutError> {
        if payouts.batch_id.is_none() || payouts.batched.is_empty() {
            return Ok(());
        }
        let mut ids = Vec::new();
        EntityEvents::<PayoutEvent>::persist(
            "bria_payout_events",
            tx,
            payouts.batched.into_iter().flat_map(|p| {
                ids.push(uuid::Uuid::from(p.id));
                p.events.into_new_serialized_events(p.id)
            }),
        )
        .await?;
        sqlx::query!(
            r#"UPDATE bria_payouts SET batch_id = $1 WHERE id = ANY($2)"#,
            payouts.batch_id.unwrap() as BatchId,
            &ids[..],
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn average_payout_per_batch(
        &self,
        wallet_id: WalletId,
        payout_queue_id: PayoutQueueId,
    ) -> Result<(usize, Satoshis), PayoutError> {
        let res = sqlx::query!(
            r#"
            SELECT 
                COALESCE(ROUND(AVG(counts)), 0) AS "average_payouts_per_batch!",
                COALESCE(ROUND(AVG(satoshis)), 0) AS "average_payout_value!"
            FROM (
                SELECT 
                    bria_payouts.batch_id,
                    COUNT(*) AS counts,
                    AVG((event->>'satoshis')::NUMERIC) AS satoshis
                FROM bria_payouts
                JOIN bria_payout_events ON bria_payouts.id = bria_payout_events.id
                WHERE bria_payouts.wallet_id = $1 AND bria_payouts.payout_queue_id = $2 AND bria_payout_events.event_type = 'initialized'
                GROUP BY bria_payouts.batch_id
            ) as subquery
            "#,
            wallet_id as WalletId,
            payout_queue_id as PayoutQueueId
        )
        .fetch_one(&self.pool)
        .await?;

        Ok((
            usize::try_from(res.average_payouts_per_batch)
                .expect("Couldn't unwrap avg_payouts_per_batch"),
            Satoshis::from(res.average_payout_value),
        ))
    }

    #[instrument(name = "payouts.find_by_id_for_cancellation", skip(self))]
    pub async fn find_by_id_for_cancellation(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: AccountId,
        payout_id: PayoutId,
    ) -> Result<Payout, PayoutError> {
        let rows = sqlx::query!(
            r#"
        SELECT b.*, e.sequence, e.event
        FROM bria_payouts b
        JOIN bria_payout_events e ON b.id = e.id
        WHERE account_id = $1 AND b.id = $2
        ORDER BY b.created_at, b.id, e.sequence
        FOR UPDATE"#,
            account_id as AccountId,
            payout_id as PayoutId,
        )
        .fetch_all(&mut **tx)
        .await?;

        if rows.is_empty() {
            return Err(PayoutError::PayoutIdNotFound(payout_id.to_string()));
        }

        let mut entity_events = EntityEvents::new();
        for row in rows {
            entity_events.load_event(row.sequence as usize, row.event)?;
        }
        Ok(Payout::try_from(entity_events)?)
    }

    pub async fn update(&self, payout: Payout) -> Result<(), PayoutError> {
        if !payout.events.is_dirty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;
        EntityEvents::<PayoutEvent>::persist(
            "bria_payout_events",
            &mut tx,
            payout.events.new_serialized_events(payout.id),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
