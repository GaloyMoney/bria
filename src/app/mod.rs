use sqlx_ledger::{account::NewAccount as NewLedgerAccount, SqlxLedger};
use uuid::Uuid;

use crate::{account::keys::*, error::*, primitives::*, wallet::*, xpub::*};

pub struct App {
    keys: AccountApiKeys,
    xpubs: XPubs,
    wallets: Wallets,
    ledger: SqlxLedger,
    pool: sqlx::PgPool,
}

impl App {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            keys: AccountApiKeys::new(&pool),
            xpubs: XPubs::new(&pool),
            wallets: Wallets::new(&pool),
            ledger: SqlxLedger::new(&pool),
            pool,
        }
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
    ) -> Result<XPubId, BriaError> {
        let id = self.xpubs.persist(account_id, name, xpub.parse()?).await?;
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

        let new_wallet = NewWallet::builder()
            .name(name.clone())
            .keychain(SingleSigWalletKeyChainConfig::new(
                xpubs.into_iter().next().unwrap(),
            ))
            .build()
            .expect("Couldn't build NewWallet");

        let mut tx = self.pool.begin().await?;
        let wallet_id = self
            .wallets
            .create_in_tx(&mut tx, account_id, new_wallet)
            .await?;
        let new_account = NewLedgerAccount::builder()
            .id(Uuid::from(wallet_id))
            .name(name)
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

    pub async fn gen_address(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<String, BriaError> {
        // let wallet = self.wallets.find(wallet_id).await?;
        // Ok(wallet)
        unimplemented!()
    }
}
