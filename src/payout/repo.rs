use es_entity::*;
use sqlx::{Pool, Postgres, Transaction};
use tracing::instrument;

use std::collections::{HashMap, HashSet};

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

    pub async fn find_by_account_id_and_id(
        &self,
        account_id: AccountId,
        id: PayoutId,
    ) -> Result<Payout, PayoutError> {
        let payout = es_entity::es_query!(
            "bria",
            &self.pool,
            r#"
            SELECT *
            FROM bria_payouts
            WHERE account_id = $1 AND id = $2"#,
            account_id as AccountId,
            id as PayoutId,
        )
        .fetch_one()
        .await?;
        Ok(payout)
    }

    #[instrument(name = "payouts.find_by_external_id", skip(self))]
    pub async fn find_by_account_id_and_external_id(
        &self,
        account_id: AccountId,
        external_id: String,
    ) -> Result<Payout, PayoutError> {
        let payout = es_entity::es_query!(
            "bria",
            &self.pool,
            r#"SELECT * FROM bria_payouts WHERE account_id = $1 AND external_id = $2"#,
            account_id as AccountId,
            external_id as String
        )
        .fetch_one()
        .await?;
        Ok(payout)
    }

    #[instrument(name = "payouts.list_unbatched", skip(self))]
    pub async fn list_unbatched(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: AccountId,
        payout_queue_id: PayoutQueueId,
    ) -> Result<UnbatchedPayouts, PayoutError> {
        let rows = sqlx::query_as!(
            GenericEvent,
            r#"
              SELECT e.recorded_at as recorded_at, e.sequence as sequence, e.id as entity_id, e.event
              FROM bria_payouts b
              JOIN bria_payout_events e ON b.id = e.id
              WHERE b.batch_id IS NULL AND b.account_id = $1 AND b.payout_queue_id = $2
              ORDER BY b.created_at, b.id, e.sequence FOR UPDATE"#,
            account_id as AccountId,
            payout_queue_id as PayoutQueueId,
        )        .fetch_all(&mut **tx)
        .await?;
        let mut count = 0;
        let mut unique_ids = HashSet::new();
        for row in &rows {
            if unique_ids.insert(row.entity_id) {
                count += 1;
            }
        }
        let (unbatched_payouts, _) = EntityEvents::load_n::<Payout>(rows, count)?;

        let mut payouts: HashMap<WalletId, Vec<Payout>> = HashMap::new();
        for payout in unbatched_payouts {
            payouts.entry(payout.wallet_id).or_default().push(payout);
        }

        let filtered_payouts: HashMap<WalletId, Vec<Payout>> = payouts
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

        let unbatched_payouts = filtered_payouts
            .into_iter()
            .map(|(wallet_id, payouts)| {
                let unbatched_payouts = payouts
                    .into_iter()
                    .filter_map(|payout| UnbatchedPayout::try_from(payout).ok())
                    .collect();
                (wallet_id, unbatched_payouts)
            })
            .collect();

        Ok(UnbatchedPayouts::new(unbatched_payouts))
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

        let value: u64 = 42;
        let size: usize = value.try_into().expect("Value too large for usize");
        // add error ?

        let payouts = es_entity::es_query!(
            "bria",
            &self.pool,
            r#"
                SELECT *
                FROM bria_payouts
                WHERE account_id = $1 AND wallet_id = $2
                ORDER BY created_at, id DESC
                OFFSET $3"#,
            account_id as AccountId,
            wallet_id as WalletId,
            offset as i64
        )
        .fetch_n(size)
        .await?;
        Ok(payouts.0)
    }

    #[instrument(name = "payouts.list_for_batch", skip(self))]
    pub async fn list_for_batch(
        &self,
        account_id: AccountId,
        batch_id: BatchId,
    ) -> Result<HashMap<WalletId, Vec<Payout>>, PayoutError> {
        let mut batched_payouts = Vec::new();
        let mut query = es_entity::PaginatedQueryArgs::<payout_cursor::PayoutsByCreatedAtCursor> {
            first: Default::default(),
            after: None,
        };

        loop {
            let es_entity::PaginatedQueryArgs { first, after } = query;
            let (id, created_at) = if let Some(after) = after {
                (Some(after.id), Some(after.created_at))
            } else {
                (None, None)
            };

            let (entities, has_next_page) = es_entity::es_query!(
                "bria",
                &self.pool,
                r#"
                SELECT *
                FROM bria_payouts
                WHERE account_id = $1 AND batch_id = $2
                AND (COALESCE((created_at, id) > ($4, $3), $3 IS NULL))
                ORDER BY created_at, id"#,
                account_id as AccountId,
                batch_id as BatchId,
                id as Option<PayoutId>,
                created_at as Option<chrono::DateTime<chrono::Utc>>,
            )
            .fetch_n(first)
            .await?;

            batched_payouts.extend(entities);

            if !has_next_page {
                break;
            }

            let end_cursor = batched_payouts
                .last()
                .map(payout_cursor::PayoutsByCreatedAtCursor::from);

            query.after = end_cursor;
        }

        let mut payouts: HashMap<WalletId, Vec<Payout>> = HashMap::new();
        for batched_payout in batched_payouts {
            payouts
                .entry(batched_payout.wallet_id)
                .or_default()
                .push(batched_payout);
        }
        Ok(payouts)
    }

    pub async fn update_unbatched(
        &self,
        tx: &mut DbOp<'_>,
        payouts: UnbatchedPayouts,
    ) -> Result<(), PayoutError> {
        if payouts.batch_id.is_none() || payouts.batched.is_empty() {
            return Ok(());
        }
        let mut ids = Vec::new();
        let mut all_events: Vec<EntityEvents<PayoutEvent>> = payouts
            .batched
            .into_iter()
            .map(|p| {
                ids.push(uuid::Uuid::from(p.id));
                p.events
            })
            .collect();

        self.persist_events_batch(tx, &mut all_events).await?;

        sqlx::query!(
            r#"UPDATE bria_payouts SET batch_id = $1 WHERE id = ANY($2)"#,
            payouts.batch_id.unwrap() as BatchId,
            &ids[..],
        )
        .execute(&mut **tx.tx())
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
        let rows = sqlx::query_as!(
            es_entity::GenericEvent,
            r#"
    SELECT e.recorded_at as recorded_at, e.sequence as sequence, e.id as entity_id, e.event
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

        let payout = EntityEvents::load_first::<Payout>(rows)?;

        Ok(payout)
    }
}
