use tracing::instrument;

use super::{error::*, keys::*};
use crate::{account::*, dev_constants, ledger::Ledger, primitives::bitcoin, profile::*};

const BOOTSTRAP_KEY_NAME: &str = "admin_bootstrap_key";

pub struct AdminApp {
    keys: AdminApiKeys,
    accounts: Accounts,
    profiles: Profiles,
    ledger: Ledger,
    pool: sqlx::PgPool,
    network: bitcoin::Network,
}

impl AdminApp {
    pub fn new(pool: sqlx::PgPool, network: bitcoin::Network) -> Self {
        Self {
            keys: AdminApiKeys::new(&pool),
            accounts: Accounts::new(&pool),
            profiles: Profiles::new(&pool),
            ledger: Ledger::new(&pool),
            pool,
            network,
        }
    }
}

impl AdminApp {
    #[instrument(name = "admin_app.dev_bootstrap", skip(self), err)]
    pub async fn dev_bootstrap(&self) -> Result<(AdminApiKey, ProfileApiKey), AdminApiError> {
        if self.network == bitcoin::Network::Bitcoin {
            return Err(AdminApiError::BadNetworkForDev);
        }
        let admin_key = self.bootstrap().await?;

        let mut tx = self.pool.begin().await?;
        let account = self
            .accounts
            .create_in_tx(&mut tx, dev_constants::DEV_ACCOUNT_NAME.to_owned())
            .await?;
        self.ledger
            .create_journal_for_account(&mut tx, account.id, account.name.clone())
            .await?;
        let profile = self
            .profiles
            .create_in_tx(&mut tx, account.id, account.name)
            .await?;
        let profile_key = self
            .profiles
            .create_key_for_profile_in_tx(&mut tx, profile, true)
            .await?;
        tx.commit().await?;
        Ok((admin_key, profile_key))
    }

    #[instrument(name = "admin_app.bootstrap", skip(self), err)]
    pub async fn bootstrap(&self) -> Result<AdminApiKey, AdminApiError> {
        self.keys.create(BOOTSTRAP_KEY_NAME.to_string()).await
    }

    #[instrument(name = "admin_app.authenticate", skip(self), err)]
    pub async fn authenticate(&self, key: &str) -> Result<(), AdminApiError> {
        self.keys.find_by_key(key).await?;
        Ok(())
    }

    #[instrument(name = "admin_app.create_account", skip(self), err)]
    pub async fn create_account(
        &self,
        account_name: String,
    ) -> Result<ProfileApiKey, AdminApiError> {
        let mut tx = self.pool.begin().await?;
        let account = self
            .accounts
            .create_in_tx(&mut tx, account_name.clone())
            .await?;
        self.ledger
            .create_journal_for_account(&mut tx, account.id, account.name.clone())
            .await?;
        let profile = self
            .profiles
            .create_in_tx(&mut tx, account.id, account.name)
            .await?;
        let key = self
            .profiles
            .create_key_for_profile_in_tx(&mut tx, profile, false)
            .await?;
        tx.commit().await?;
        Ok(key)
    }

    #[instrument(name = "admin_app.list_accounts", skip(self), err)]
    pub async fn list_accounts(&self) -> Result<Vec<Account>, AdminApiError> {
        Ok(self.accounts.list().await?)
    }
}
