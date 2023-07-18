use sqlx::{Pool, Postgres, Transaction};

use super::{entity::*, error::DescriptorError};
use crate::primitives::*;

#[derive(Clone)]
pub struct Descriptors {
    _pool: Pool<Postgres>,
}

impl Descriptors {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self {
            _pool: pool.clone(),
        }
    }

    pub async fn persist_all_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        descriptors: Vec<NewDescriptor>,
    ) -> Result<(), DescriptorError> {
        for descriptor in descriptors {
            self.persist_in_tx(tx, descriptor).await?;
        }
        Ok(())
    }

    async fn persist_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        descriptor: NewDescriptor,
    ) -> Result<(), DescriptorError> {
        let (descriptor_str, checksum) = descriptor.descriptor_and_checksum();
        let res = sqlx::query!(
            r#"WITH ins AS (
                   INSERT INTO bria_descriptors (id, account_id, wallet_id, descriptor, checksum, kind)
                   VALUES ($1, $2, $3, $4, $5, $6)
                   ON CONFLICT (account_id, checksum) DO NOTHING
                   RETURNING wallet_id
               )
               SELECT wallet_id AS "wallet_id: WalletId" FROM ins
               UNION ALL
               SELECT wallet_id FROM bria_descriptors
               WHERE account_id = $2 AND checksum = $5
               LIMIT 1;
               "#,
            descriptor.db_uuid,
            descriptor.account_id as AccountId,
            descriptor.wallet_id as WalletId,
            descriptor_str,
            checksum,
            bitcoin::pg::PgKeychainKind::from(descriptor.keychain_kind)
                as bitcoin::pg::PgKeychainKind,
        )
        .fetch_one(&mut **tx)
        .await?;

        if res.wallet_id != Some(descriptor.wallet_id) {
            return Err(DescriptorError::DescriptorAlreadyInUse);
        }
        Ok(())
    }
}
