use sqlx_ledger::journal::*;
use uuid::Uuid;

use crate::account::{keys::*, *};

use super::{error::*, keys::*};

const BOOTSTRAP_KEY_NAME: &str = "admin_bootstrap_key";

pub struct AdminApp {
    keys: AdminApiKeys,
    accounts: Accounts,
    account_keys: AccountApiKeys,
    journals: Journals,
    pool: sqlx::PgPool,
}

impl AdminApp {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            keys: AdminApiKeys::new(&pool),
            accounts: Accounts::new(&pool),
            account_keys: AccountApiKeys::new(&pool),
            journals: Journals::new(&pool),
            pool,
        }
    }
}

impl AdminApp {
    pub async fn bootstrap(&self) -> Result<AdminApiKey, AdminApiError> {
        self.keys.create(BOOTSTRAP_KEY_NAME.to_string()).await
    }

    pub async fn authenticate(&self, key: &str) -> Result<(), AdminApiError> {
        self.keys.find_by_key(key).await?;
        Ok(())
    }

    pub async fn create_account(&self, name: String) -> Result<AccountApiKey, AdminApiError> {
        let mut tx = self.pool.begin().await?;
        let account = self.accounts.create_in_tx(&mut tx, name.clone()).await?;
        let new_journal = NewJournal::builder()
            .id(Uuid::from(account.id))
            .name(name.clone())
            .build()
            .expect("Couldn't build NewJournal");
        self.journals.create_in_tx(&mut tx, new_journal).await?;
        let keys = self
            .account_keys
            .create_in_tx(&mut tx, name, account.id)
            .await?;
        tx.commit().await?;
        Ok(keys)
    }
}
