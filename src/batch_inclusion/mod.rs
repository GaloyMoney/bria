pub mod error;

use chrono::{DateTime, Duration, Utc};

use std::collections::HashMap;

use crate::{
    payout::Payout,
    payout_queue::{PayoutQueue, PayoutQueues},
    primitives::*,
};

use error::BatchInclusionError;

type BatchInclusionEstimate = DateTime<Utc>;

pub struct PayoutWithInclusionEstimate {
    pub payout: Payout,
    pub estimated_batch_inclusion: Option<BatchInclusionEstimate>,
}

impl From<(Payout, Option<&BatchInclusionEstimate>)> for PayoutWithInclusionEstimate {
    fn from(
        (payout, estimated_batch_inclusion): (Payout, Option<&BatchInclusionEstimate>),
    ) -> Self {
        let estimate = if payout.batch_id.is_some() || payout.is_cancelled() {
            None
        } else {
            estimated_batch_inclusion
        };
        Self {
            payout,
            estimated_batch_inclusion: estimate.copied(),
        }
    }
}

#[derive(Clone)]
pub struct BatchInclusion {
    pool: sqlx::PgPool,
    payout_queues: PayoutQueues,
}

impl BatchInclusion {
    pub fn new(pool: sqlx::PgPool, payout_queues: PayoutQueues) -> Self {
        Self {
            payout_queues,
            pool,
        }
    }

    pub async fn estimate_next_queue_trigger(
        &self,
        payout_queue: PayoutQueue,
    ) -> Result<Option<BatchInclusionEstimate>, BatchInclusionError> {
        let id = payout_queue.id;
        let mut next_queue_trigger_times =
            self.next_queue_trigger_times(vec![payout_queue]).await?;
        Ok(next_queue_trigger_times.remove(&id))
    }

    pub async fn include_estimate(
        &self,
        account_id: AccountId,
        payout: Payout,
    ) -> Result<PayoutWithInclusionEstimate, BatchInclusionError> {
        if payout.batch_id.is_some() || payout.is_cancelled() {
            return Ok(PayoutWithInclusionEstimate::from((payout, None)));
        }
        let queue = self
            .payout_queues
            .find_by_id(account_id, payout.payout_queue_id)
            .await?;
        let estimate = self.estimate_next_queue_trigger(queue).await?;
        Ok(PayoutWithInclusionEstimate {
            estimated_batch_inclusion: estimate,
            payout,
        })
    }

    pub async fn include_estimates(
        &self,
        account_id: AccountId,
        payouts: Vec<Payout>,
    ) -> Result<Vec<PayoutWithInclusionEstimate>, BatchInclusionError> {
        let queues = self.payout_queues.list_by_account_id(account_id).await?;
        let next_queue_trigger_times = self.next_queue_trigger_times(queues).await?;
        Ok(payouts
            .into_iter()
            .map(|payout| {
                let estimate = next_queue_trigger_times.get(&payout.payout_queue_id);
                PayoutWithInclusionEstimate::from((payout, estimate))
            })
            .collect())
    }

    async fn next_queue_trigger_times(
        &self,
        queues: Vec<PayoutQueue>,
    ) -> Result<HashMap<PayoutQueueId, BatchInclusionEstimate>, BatchInclusionError> {
        let queue_ids = queues.iter().map(|q| uuid::Uuid::from(q.id)).collect();
        let next_attempts = Self::next_attempt_of_queues(&self.pool, queue_ids).await?;

        let mut res = HashMap::new();
        for queue in queues.into_iter() {
            if let Some(next_attempt) = next_attempts.get(&queue.id) {
                res.insert(queue.id, *next_attempt);
            } else if let Some(interval) = queue.spawn_in() {
                res.insert(
                    queue.id,
                    Utc::now()
                        + Duration::from_std(interval)
                            .expect("interval value will always be less than i64"),
                );
            }
        }
        Ok(res)
    }

    async fn next_attempt_of_queues(
        pool: &sqlx::PgPool,
        ids: Vec<uuid::Uuid>,
    ) -> Result<HashMap<PayoutQueueId, chrono::DateTime<chrono::Utc>>, BatchInclusionError> {
        let result = sqlx::query!(
            "SELECT id, attempt_at FROM mq_msgs WHERE id = ANY($1)",
            &ids
        )
        .fetch_all(pool)
        .await?;
        let mut map = HashMap::new();
        for row in result {
            if let Some(attempt_at) = row.attempt_at {
                map.insert(
                    PayoutQueueId::from(row.id),
                    attempt_at
                        + chrono::Duration::try_seconds(1).expect("could not convert to duration"),
                );
            }
        }
        Ok(map)
    }
}
