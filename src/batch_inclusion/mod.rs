pub mod error;

use chrono::{DateTime, Duration, Utc};

use std::collections::HashMap;

use crate::{
    job,
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

    pub async fn include_estimation(
        &self,
        account_id: AccountId,
        payouts: Vec<Payout>,
    ) -> Result<Vec<PayoutWithInclusionEstimate>, BatchInclusionError> {
        let queues = self.payout_queues.list_by_account_id(account_id).await?;
        let next_queue_trigger_times = self.next_queue_trigger_times(queues).await?;
        Ok(payouts
            .into_iter()
            .map(|payout| PayoutWithInclusionEstimate {
                estimated_batch_inclusion: next_queue_trigger_times
                    .get(&payout.payout_queue_id)
                    .copied(),
                payout,
            })
            .collect())
    }

    async fn next_queue_trigger_times(
        &self,
        queues: Vec<PayoutQueue>,
    ) -> Result<HashMap<PayoutQueueId, BatchInclusionEstimate>, BatchInclusionError> {
        let queue_ids = queues.iter().map(|q| q.id).collect();
        let next_attempts = job::next_attempt_of_queues(&self.pool, queue_ids).await?;

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
}
