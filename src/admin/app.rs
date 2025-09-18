use tracing::instrument;

use super::{error::*, keys::*};
use crate::{account::*, dev_constants, ledger::Ledger, primitives::bitcoin, profile::*};

const BOOTSTRAP_KEY_NAME: &str = "admin_bootstrap_key";

pub struct AdminApp {
    keys: AdminApiKeys,
    accounts: Accounts,
    profiles: Profiles,
    ledger: Ledger,
    network: bitcoin::Network,
}

impl AdminApp {
    pub fn new(pool: sqlx::PgPool, network: bitcoin::Network) -> Self {
        Self {
            keys: AdminApiKeys::new(&pool),
            accounts: Accounts::new(&pool),
            profiles: Profiles::new(&pool),
            ledger: Ledger::new(&pool),
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
        let mut op = self.profiles.begin_op().await?;

        let account = self
            .accounts
            .create_in_op(&mut op, dev_constants::DEV_ACCOUNT_NAME.to_owned())
            .await?;
        self.ledger
            .create_journal_for_account(op.tx_mut(), account.id, account.name.clone())
            .await?;
        let new_profile = NewProfile::builder()
            .account_id(account.id)
            .name(account.name)
            .build()
            .expect("Couldn't build NewProfile");
        let profile = self.profiles.create_in_op(&mut op, new_profile).await?;
        let profile_key = self
            .profiles
            .create_key_for_profile_in_op(&mut op, profile, true)
            .await?;
        op.commit().await?;
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
        let mut op = self.profiles.begin_op().await?;
        let account = self
            .accounts
            .create_in_op(&mut op, account_name.clone())
            .await?;
        self.ledger
            .create_journal_for_account(op.tx_mut(), account.id, account.name.clone())
            .await?;
        let new_profile = NewProfile::builder()
            .account_id(account.id)
            .name(account.name)
            .build()
            .expect("Couldn't build NewProfile");
        let profile = self.profiles.create_in_op(&mut op, new_profile).await?;
        let key = self
            .profiles
            .create_key_for_profile_in_op(&mut op, profile, false)
            .await?;
        op.commit().await?;
        Ok(key)
    }

    #[instrument(name = "admin_app.list_accounts", skip(self), err)]
    pub async fn list_accounts(&self) -> Result<Vec<Account>, AdminApiError> {
        Ok(self.accounts.list().await?)
    }
}
