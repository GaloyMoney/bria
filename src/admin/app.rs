use sqlx_ledger::journal::*;

use crate::account::{keys::*, *};

use super::{error::*, keys::*};

const BOOTSTRAP_KEY_NAME: &str = "admin_bootstrap_key";

pub struct AdminApp {
    keys: AdminApiKeys,
    accounts: Accounts,
    account_keys: AccountApiKeys,
    journals: Journals,
}

impl AdminApp {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            keys: AdminApiKeys::new(&pool),
            accounts: Accounts::new(&pool),
            account_keys: AccountApiKeys::new(&pool),
            journals: Journals::new(&pool),
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
        let new_journal = NewJournal::builder()
            .name(name.clone())
            .build()
            .expect("Invalid journal name");
        let journal_id = self.journals.create(new_journal).await?;
        let account = self.accounts.create(name.clone(), journal_id).await?;
        Ok(self.account_keys.create(name, account.id).await?)
    }
}
