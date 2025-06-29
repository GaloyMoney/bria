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

    pub async fn find_by_account_id_and_id(
        &self,
        account_id: AccountId,
        id: PayoutQueueId,
    ) -> Result<PayoutQueue, PayoutQueueError> {
        let payout_queue = es_entity::es_query!(
            "bria",
            &self.pool,
            r#"
            SELECT *
            FROM bria_payout_queues
            WHERE account_id = $1 and id = $2"#,
            account_id as AccountId,
            id as PayoutQueueId,
        )
        .fetch_one()
        .await?;

        Ok(payout_queue)
    }

    pub async fn find_by_account_id_and_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<PayoutQueue, PayoutQueueError> {
        let payout_queue = es_entity::es_query!(
            "bria",
            &self.pool,
            r#"
            SELECT *
            FROM bria_payout_queues
            WHERE account_id = $1 and name = $2"#,
            account_id as AccountId,
            name as String,
        )
        .fetch_one()
        .await?;
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
