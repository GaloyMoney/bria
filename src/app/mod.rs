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
            .keychain(SingleSigWalletKeyChainConfig::new(
                xpubs.into_iter().next().unwrap(),
            ))
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
