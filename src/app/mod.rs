mod config;

use sqlx_ledger::{account::NewAccount as NewLedgerAccount, SqlxLedger};
use sqlxmq::OwnedHandle;
use uuid::Uuid;

pub use config::*;

use crate::{account::keys::*, error::*, job, primitives::*, wallet::*, xpub::*};

pub struct App {
    _runner: OwnedHandle,
    keys: AccountApiKeys,
    xpubs: XPubs,
    wallets: Wallets,
    ledger: SqlxLedger,
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
        let runner = job::start_job_runner(
            &pool,
            wallets.clone(),
            wallets_cfg.sync_all_delay,
            blockchain_cfg.network,
        )
        .await?;
        Self::spawn_sync_all_wallets(pool.clone(), wallets_cfg.sync_all_delay).await?;
        Ok(Self {
            keys: AccountApiKeys::new(&pool),
            xpubs: XPubs::new(&pool),
            wallets,
            ledger: SqlxLedger::new(&pool),
            pool,
            _runner: runner,
            blockchain_cfg,
        })
    }

    pub async fn authenticate(&self, key: &str) -> Result<AccountId, BriaError> {
        let key = self.keys.find_by_key(key).await?;
        Ok(key.account_id)
    }

    pub async fn import_xpub(
        &self,
        account_id: AccountId,
        name: String,
        xpub: String,
        derivation: Option<String>,
    ) -> Result<XPubId, BriaError> {
        let xpub = XPub::try_from((xpub, derivation))?;
        let id = self.xpubs.persist(account_id, name, xpub).await?;
        Ok(id)
    }

    pub async fn create_wallet(
        &self,
        account_id: AccountId,
        name: String,
        xpub_refs: Vec<String>,
    ) -> Result<WalletId, BriaError> {
        let mut xpubs = Vec::new();
        for xpub_ref in xpub_refs {
            xpubs.push(self.xpubs.find_from_ref(account_id, xpub_ref).await?);
        }

        if xpubs.len() > 1 {
            unimplemented!()
        }

        let wallet_id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        let dust_account = NewLedgerAccount::builder()
            .name(format!("{}-dust", wallet_id))
            .code(format!("WALLET_{}_DUST", wallet_id))
            .build()
            .expect("Couldn't build NewLedgerAccount");
        let dust_account_id = self
            .ledger
            .accounts()
            .create_in_tx(&mut tx, dust_account)
            .await?;
        let new_wallet = NewWallet::builder()
            .id(wallet_id)
            .name(name.clone())
            .keychain(WpkhKeyChainConfig::new(xpubs.into_iter().next().unwrap()))
            .dust_account_id(dust_account_id)
            .build()
            .expect("Couldn't build NewWallet");
        let wallet_id = self
            .wallets
            .create_in_tx(&mut tx, account_id, new_wallet)
            .await?;
        let new_account = NewLedgerAccount::builder()
            .id(Uuid::from(wallet_id))
            .name(wallet_id.to_string())
            .code(format!("WALLET_{}", wallet_id))
            .build()
            .expect("Couldn't build NewLedgerAccount");
        self.ledger
            .accounts()
            .create_in_tx(&mut tx, new_account)
            .await?;

        tx.commit().await?;

        Ok(wallet_id)
    }

    pub async fn new_address(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<String, BriaError> {
        let wallet = self.wallets.find_by_name(account_id, name).await?;
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
