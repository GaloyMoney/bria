pub mod error;

use chrono::{DateTime, Duration, Utc};

use std::collections::HashMap;

use crate::{job, payout::Payout, payout_queue::PayoutQueue, primitives::*};

use error::BatchInclusionError;

type BatchInclusionEstimate = DateTime<Utc>;

pub struct PayoutWithInclusionEstimate {
    pub payout: Payout,
    pub estimated_batch_inclusion: Option<BatchInclusionEstimate>,
}

pub struct BatchInclusion {
    pool: sqlx::PgPool,
}

impl BatchInclusion {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub async fn estimate_for_payout(
        &self,
        payout_id: PayoutId,
        payout_queue: PayoutQueue,
    ) -> Result<Option<BatchInclusionEstimate>, BatchInclusionError> {
        let payouts = [(payout_id, payout_queue.id)].into_iter().collect();
        let queues = [(payout_queue.id, payout_queue)].into_iter().collect();
        Ok(self
            .estimate_batch_inclusion(queues, payouts)
            .await?
            .remove(&payout_id))
    }

    async fn estimate_batch_inclusion(
        &self,
        queues: HashMap<PayoutQueueId, PayoutQueue>,
        payouts: HashMap<PayoutId, PayoutQueueId>,
    ) -> Result<HashMap<PayoutId, BatchInclusionEstimate>, BatchInclusionError> {
        let queue_ids = queues.keys().copied().collect();
        let next_attempts = job::next_attempt_of_queues(&self.pool, queue_ids).await?;

        let mut res = HashMap::new();
        for (payout_id, payout_queue_id) in payouts.into_iter() {
            if let Some(next_attempt) = next_attempts.get(&payout_queue_id) {
                res.insert(payout_id, *next_attempt);
            } else if let Some(interval) = queues.get(&payout_queue_id).and_then(|p| p.spawn_in()) {
                res.insert(
                    payout_id,
                    Utc::now()
                        + Duration::from_std(interval)
                            .expect("interval value will always be less than i64"),
                );
            }
        }
        Ok(res)
    }
}
