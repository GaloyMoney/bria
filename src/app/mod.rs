use crate::{account::keys::*, error::*, primitives::*, wallet::*, xpub::*};
use sqlx_ledger::{account::NewAccount as NewLedgerAccount, SqlxLedger};

pub struct App {
    keys: AccountApiKeys,
    xpubs: XPubs,
    wallets: Wallets,
    ledger: SqlxLedger,
}

impl App {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            keys: AccountApiKeys::new(&pool),
            xpubs: XPubs::new(&pool),
            wallets: Wallets::new(&pool),
            ledger: SqlxLedger::new(&pool),
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
        let id = self.xpubs.persist(account_id, name, xpub).await?;
        Ok(id)
    }

    pub async fn create_wallet(
        &self,
        account_id: AccountId,
        name: String,
        mut xpub_ids: Vec<String>,
    ) -> Result<WalletId, BriaError> {
        let mut xpub_ids = xpub_ids.drain(..).map(|id| id.parse());
        let xpub = self
            .xpubs
            .find(account_id, xpub_ids.next().unwrap()?)
            .await?;

        let new_wallet = NewWallet::builder()
            .name(name.clone())
            .keychain(SingleSigWalletKeyChainConfig::new(xpub))
            .build()
            .expect("Couldn't build NewWallet");
        let new_account = NewLedgerAccount::builder()
            .name(name)
            .code(format!("WALLET_{}", new_wallet.id))
            .build()
            .expect("Couldn't build NewLedgerAccount");
        let new_account_id = self.ledger.accounts().create(new_account).await?;

        let wallet_id = self
            .wallets
            .create(account_id, new_account_id, new_wallet)
            .await?;
        Ok(wallet_id)
    }
}
