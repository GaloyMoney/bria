mod config;

use sqlx_ledger::balance::AccountBalance as LedgerAccountBalance;
use sqlxmq::OwnedHandle;
use tracing::instrument;

pub use config::*;

use crate::{account::keys::*, error::*, job, ledger::Ledger, primitives::*, wallet::*, xpub::*};

pub struct App {
    _runner: OwnedHandle,
    keys: AccountApiKeys,
    xpubs: XPubs,
    wallets: Wallets,
    ledger: Ledger,
    pool: sqlx::PgPool,
    blockchain_cfg: BlockchainConfig,
}

impl App {
    pub async fn run(
        pool: sqlx::PgPool,
        blockchain_cfg: BlockchainConfig,
        wallets_cfg: WalletsConfig,
    ) -> Result<Self, BriaError> {
        let wallets = Wallets::new(&pool);
        let ledger = Ledger::init(&pool).await?;
        let runner = job::start_job_runner(
            &pool,
            wallets.clone(),
            ledger.clone(),
            wallets_cfg.sync_all_delay,
            blockchain_cfg.clone(),
        )
        .await?;
        Self::spawn_sync_all_wallets(pool.clone(), wallets_cfg.sync_all_delay).await?;
        Ok(Self {
            keys: AccountApiKeys::new(&pool),
            xpubs: XPubs::new(&pool),
            wallets,
            pool,
            ledger,
            _runner: runner,
            blockchain_cfg,
        })
    }

    #[instrument(name = "app.authenticate", skip_all, err)]
    pub async fn authenticate(&self, key: &str) -> Result<AccountId, BriaError> {
        let key = self.keys.find_by_key(key).await?;
        Ok(key.account_id)
    }

    #[instrument(name = "app.import_xpub", skip(self), err)]
    pub async fn import_xpub(
        &self,
        account_id: AccountId,
        key_name: String,
        xpub: String,
        derivation: Option<String>,
    ) -> Result<XPubId, BriaError> {
        let xpub = XPub::try_from((xpub, derivation))?;
        let id = self.xpubs.persist(account_id, key_name, xpub).await?;
        Ok(id)
    }

    #[instrument(name = "app.create_wallet", skip(self), err)]
    pub async fn create_wallet(
        &self,
        account_id: AccountId,
        wallet_name: String,
        xpub_refs: Vec<String>,
    ) -> Result<WalletId, BriaError> {
        let mut xpubs = Vec::new();
        for xpub_ref in xpub_refs {
            xpubs.push(self.xpubs.find_from_ref(account_id, xpub_ref).await?);
        }

        if xpubs.len() > 1 {
            unimplemented!()
        }

        let wallet_id = WalletId::new();
        let mut tx = self.pool.begin().await?;
        let dust_account_id = self
            .ledger
            .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &wallet_name)
            .await?;
        let new_wallet = NewWallet::builder()
            .id(wallet_id)
            .name(wallet_name.clone())
            .keychain(WpkhKeyChainConfig::new(xpubs.into_iter().next().unwrap()))
            .dust_account_id(dust_account_id)
            .build()
            .expect("Couldn't build NewWallet");
        let wallet_id = self
            .wallets
            .create_in_tx(&mut tx, account_id, new_wallet)
            .await?;

        tx.commit().await?;

        Ok(wallet_id)
    }

    #[instrument(name = "app.get_wallet_balance", skip(self), err)]
    pub async fn get_wallet_balance(
        &self,
        account_id: AccountId,
        wallet_name: String,
    ) -> Result<Option<LedgerAccountBalance>, BriaError> {
        let wallet = self.wallets.find_by_name(account_id, wallet_name).await?;
        Ok(self
            .ledger
            .get_balance(wallet.journal_id, wallet.ledger_account_id)
            .await?)
    }

    #[instrument(name = "app.new_address", skip(self), err)]
    pub async fn new_address(
        &self,
        account_id: AccountId,
        wallet_name: String,
    ) -> Result<String, BriaError> {
        let wallet = self.wallets.find_by_name(account_id, wallet_name).await?;
        let (keychain_id, cfg) = wallet.current_keychain();
        let keychain_wallet = KeychainWallet::new(
            self.pool.clone(),
            self.blockchain_cfg.network,
            keychain_id,
            cfg.clone(),
        );
        let addr = keychain_wallet.new_external_address().await?;
        Ok(addr.to_string())
    }

    #[instrument(name = "app.spawn_sync_all_wallets", skip_all, err)]
    async fn spawn_sync_all_wallets(
        pool: sqlx::PgPool,
        delay: std::time::Duration,
    ) -> Result<(), BriaError> {
        let _ = tokio::spawn(async move {
            loop {
                let _ = job::spawn_sync_all_wallets(&pool, std::time::Duration::from_secs(1)).await;
                tokio::time::sleep(delay).await;
            }
        });
        Ok(())
    }
}
