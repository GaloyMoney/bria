use es_entity::*;
use sqlx::{Pool, Postgres};

use super::{entity::*, error::PayoutQueueError};

use crate::primitives::*;

#[derive(EsRepo, Clone, Debug)]
#[es_repo(
    entity = "PayoutQueue",
    err = "PayoutQueueError",
    columns(name(ty = "String"), account_id(ty = "AccountId", list_for)),
    tbl_prefix = "bria"
)]
pub struct PayoutQueues {
    pool: Pool<Postgres>,
}

impl PayoutQueues {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn find_by_id_and_account_id(
        &self,
        id: PayoutQueueId,
        account_id: AccountId,
    ) -> Result<PayoutQueue, PayoutQueueError> {
        let payout_queue = self.find_by_id(id).await?;

        if payout_queue.account_id != account_id {
            return Err(PayoutQueueError::EsEntityError(EsEntityError::NotFound));
        }
        Ok(payout_queue)
    }

    pub async fn find_by_name_and_account_id(
        &self,
        name: String,
        account_id: AccountId,
    ) -> Result<PayoutQueue, PayoutQueueError> {
        let payout_queue = self.find_by_name(name).await?;

        if payout_queue.account_id != account_id {
            return Err(PayoutQueueError::EsEntityError(EsEntityError::NotFound));
        }
        Ok(payout_queue)
    }

    pub async fn list_for_account_id(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<PayoutQueue>, PayoutQueueError> {
        let mut queues = Vec::new();
        let mut query = Default::default();

        loop {
            let mut paginated_queues = self
                .list_for_account_id_by_id(account_id, query, Default::default())
                .await?;
            queues.append(&mut paginated_queues.entities);
            if let Some(q) = paginated_queues.into_next_query() {
                query = q;
            } else {
                break;
            };
        }
        Ok(queues)
    }

    pub async fn list_all(&self) -> Result<Vec<PayoutQueue>, PayoutQueueError> {
        let mut queues = Vec::new();
        let mut query = Default::default();

        loop {
            let mut paginated_queues = self.list_by_id(query, Default::default()).await?;
            queues.append(&mut paginated_queues.entities);
            if let Some(q) = paginated_queues.into_next_query() {
                query = q;
            } else {
                break;
            };
        }
        Ok(queues)
    }
}
