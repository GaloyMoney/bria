use sqlx::{Pool, Postgres};

use super::{entity::*, error::PayoutQueueError};
use crate::primitives::*;
use es_entity::*;

#[derive(EsRepo, Clone, Debug)]
#[es_repo(
    entity = "PayoutQueue",
    err = "PayoutQueueError",
    columns(name(ty = "String",), account_id(ty = "AccountId", list_for)),
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
}
