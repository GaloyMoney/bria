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
        let mut next = Some(PaginatedQueryArgs::default());

        while let Some(query) = next.take() {
            let mut ret = self
                .list_for_account_id_by_created_at(account_id, query, Default::default())
                .await?;

            queues.append(&mut ret.entities);
            next = ret.into_next_query();
        }

        Ok(queues)
    }

    pub async fn list_all(&self) -> Result<Vec<PayoutQueue>, PayoutQueueError> {
        let mut queues = Vec::new();
        let mut next = Some(PaginatedQueryArgs::default());

        while let Some(query) = next.take() {
            let mut ret = self.list_by_id(query, Default::default()).await?;

            queues.append(&mut ret.entities);
            next = ret.into_next_query();
        }

        Ok(queues)
    }
}
