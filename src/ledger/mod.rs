use sqlx::PgPool;
use sqlx_ledger::{
    account::NewAccount as NewLedgerAccount, journal::*, AccountId as LedgerAccountId, JournalId,
    SqlxLedger,
};
use uuid::Uuid;

use crate::{error::*, primitives::*};

pub struct Ledger {
    inner: SqlxLedger,
}

impl Ledger {
    pub fn new(pool: &PgPool) -> Self {
        Self {
            inner: SqlxLedger::new(pool),
        }
    }

    pub async fn init(pool: &PgPool) -> Result<Self, BriaError> {
        let inner = SqlxLedger::new(&pool);
        Ok(Self { inner })
    }

    pub async fn create_journal_for_account(
        &self,
        mut tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: AccountId,
        name: String,
    ) -> Result<JournalId, BriaError> {
        let new_journal = NewJournal::builder()
            .id(Uuid::from(id))
            .name(name.clone())
            .build()
            .expect("Couldn't build NewJournal");
        let id = self
            .inner
            .journals()
            .create_in_tx(&mut tx, new_journal)
            .await?;
        Ok(id)
    }

    pub async fn create_ledger_accounts_for_wallet(
        &self,
        mut tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        wallet_id: WalletId,
    ) -> Result<LedgerAccountId, BriaError> {
        let dust_account = NewLedgerAccount::builder()
            .name(format!("{}-dust", wallet_id))
            .code(format!("WALLET_{}_DUST", wallet_id))
            .build()
            .expect("Couldn't build NewLedgerAccount");
        let dust_account_id = self
            .inner
            .accounts()
            .create_in_tx(&mut tx, dust_account)
            .await?;
        let new_account = NewLedgerAccount::builder()
            .id(Uuid::from(wallet_id))
            .name(wallet_id.to_string())
            .code(format!("WALLET_{}", wallet_id))
            .build()
            .expect("Couldn't build NewLedgerAccount");
        self.inner
            .accounts()
            .create_in_tx(&mut tx, new_account)
            .await?;
        Ok(dust_account_id)
    }
}
