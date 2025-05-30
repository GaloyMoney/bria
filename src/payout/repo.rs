use anyhow::Ok;
use es_entity::*;
use sqlx::{query_file_as, Pool, Postgres, Transaction};
use tracing::instrument;

use std::{collections::HashMap, thread::panicking};

use super::{entity::*, error::*, unbatched::*};
use crate::primitives::*;

#[derive(EsRepo, Clone, Debug)]
#[es_repo(
    entity = "Payout",
    err = "PayoutError",
    columns(account_id(ty = "AccountId", list_for, update(persist = false)),),
    tbl_prefix = "bria"
)]
pub struct Payouts {
    pool: Pool<Postgres>,
}

impl Payouts {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    // #[instrument(name = "payouts.create", skip(self, tx))]
    // pub async fn create_in_tx(
    //     &self,
    //     tx: &mut Transaction<'_, Postgres>,
    //     new_payout: NewPayout,
    // ) -> Result<PayoutId, PayoutError> {
    //     sqlx::query!(
    //         r#"INSERT INTO bria_payouts (id, account_id, wallet_id, payout_queue_id, profile_id, external_id)
    //            VALUES ($1, $2, $3, $4, $5, $6)"#,
    //         new_payout.id as PayoutId,
    //         new_payout.account_id as AccountId,
    //         new_payout.wallet_id as WalletId,
    //         new_payout.payout_queue_id as PayoutQueueId,
    //         new_payout.profile_id as ProfileId,
    //         new_payout.external_id,
    //     ).execute(&mut **tx).await?;
    //     let id = new_payout.id;
    //     EntityEvents::<PayoutEvent>::persist(
    //         "bria_payout_events",
    //         tx,
    //         new_payout.initial_events().new_serialized_events(id),
    //     )
    //     .await?;
    //     Ok(id)
    // }

    pub async fn find_by_account_id_and_id(
        &self,
        account_id: AccountId,
        id: PayoutId,
    ) -> Result<Payout, PayoutError> {
        let mut payouts = Vec::new();
        let mut next = Some(PaginatedQueryArgs::default());

        while let Some(query) = next.take() {
            let mut ret = self
                .list_for_account_id_by_id(account_id, query, Default::default())
                .await?;
            payouts.append(&mut ret.entities);
            next = ret.into_next_query();
        }
        payouts
            .into_iter()
            .find(|payout| payout.id == id)
            .ok_or(PayoutError::EsEntityError(EsEntityError::NotFound))
    }
    
    // #[instrument(name = "payouts.find_by_id", skip(self))]
    // pub async fn find_by_id(
    //     &self,
    //     account_id: AccountId,
    //     payout_id: PayoutId,
    // ) -> Result<Payout, PayoutError> {
    //     let rows = sqlx::query!(
    //         r#"
    //       SELECT b.*, e.sequence, e.event
    //       FROM bria_payouts b
    //       JOIN bria_payout_events e ON b.id = e.id
    //       WHERE account_id = $1 AND b.id = $2
    //       ORDER BY b.created_at, b.id, e.sequence"#,
    //         account_id as AccountId,
    //         payout_id as PayoutId,
    //     )
    //     .fetch_all(&self.pool)
    //     .await?;

    //     if rows.is_empty() {
    //         return Err(PayoutError::PayoutIdNotFound(payout_id.to_string()));
    //     }

    //     let mut entity_events = EntityEvents::new();
    //     for row in rows {
    //         entity_events.load_event(row.sequence as usize, row.event)?;
    //     }
    //     Ok(Payout::try_from(entity_events)?)
    // }

    #[instrument(name = "payouts.find_by_external_id", skip(self))]
    // order by in each, list by, list for
    pub async fn find_by_account_id_and_external_id(
        &self,
        account_id: AccountId,
        external_id: String,
    ) -> Result<Payout, PayoutError> {
        let mut payouts = Vec::new();
        let mut next = Some(PaginatedQueryArgs::default());

        while let Some(query) = next.take() {
            let mut ret = self
                .list_for_account_id_by_id(account_id, query, Default::default())
                .await?;
            payouts.append(&mut ret.entities);
            next = ret.into_next_query();
        }
        payouts
            .into_iter()
            .find(|payout| payout.external_id == external_id)
            .ok_or(PayoutError::EsEntityError(EsEntityError::NotFound))
    }

    #[instrument(name = "payouts.list_unbatched", skip(self))]
    pub async fn list_unbatched(
        &self,
        tx: &mut Transaction<'_, Postgres>,
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
        .fetch_all(&mut **tx)
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
                            .iter_all()
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
        page: u64,
        page_size: u64,
    ) -> Result<Vec<Payout>, PayoutError> {
        let offset = (page - 1) * page_size;

        let rows = sqlx::query!(
            r#"
            WITH payouts AS (
            SELECT *
            FROM bria_payouts
            WHERE account_id = $1 AND wallet_id = $2
            ORDER BY created_at DESC, id
            LIMIT $3 OFFSET $4
            )
            SELECT p.*, e.sequence, e.event
            FROM payouts p
            JOIN bria_payout_events e ON p.id = e.id
            ORDER BY p.created_at DESC, p.id, e.sequence
            "#,
            account_id as AccountId,
            wallet_id as WalletId,
            page_size as i64,
            offset as i64,
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
                payouts.push(Payout::try_from_events(events)?);
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
                let payout = Payout::try_from_events(events)?;
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
            return Err(PayoutError::EsEntityError(EsEntityError::NotFound));
        }

        let mut entity_events = EntityEvents::<PayoutEvent>::init(payout_id, vec![]);
        for row in rows {
            entity_events.push(row.event as PayoutEvent);
        }
        Ok(Payout::try_from_events(entity_events)?)
    }

    // pub async fn update(
    //     &self,
    //     tx: &mut Transaction<'_, Postgres>,
    //     payout: Payout,
    // ) -> Result<(), PayoutError> {
    //     if !payout.events.is_dirty() {
    //         return Ok(());
    //     }
    //     EntityEvents::<PayoutEvent>::persist(
    //         "bria_payout_events",
    //         tx,
    //         payout.events.new_serialized_events(payout.id),
    //     )
    //     .await?;
    //     Ok(())
    // }
}
