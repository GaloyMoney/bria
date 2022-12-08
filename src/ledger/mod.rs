mod constants;
mod templates;

use sqlx::{PgPool, Postgres, Transaction};
use sqlx_ledger::{
    account::NewAccount as NewLedgerAccount, balance::AccountBalance as LedgerAccountBalance,
    journal::*, AccountId as LedgerAccountId, Currency, DebitOrCredit, JournalId, SqlxLedger,
    SqlxLedgerError,
};
use tracing::instrument;
use uuid::Uuid;

use crate::{error::*, primitives::*};
use constants::*;
pub use templates::*;

#[derive(Debug, Clone)]
pub struct Ledger {
    inner: SqlxLedger,
    btc: Currency,
}

impl Ledger {
    pub fn new(pool: &PgPool) -> Self {
        Self {
            inner: SqlxLedger::new(pool),
            btc: "BTC".parse().unwrap(),
        }
    }

    pub async fn init(pool: &PgPool) -> Result<Self, BriaError> {
        let inner = SqlxLedger::new(pool);
        Self::onchain_income_account(&inner).await?;
        templates::IncomingUtxo::init(&inner).await?;
        templates::ConfirmedUtxo::init(&inner).await?;
        templates::QueuedPayout::init(&inner).await?;
        Ok(Self {
            inner,
            btc: "BTC".parse().unwrap(),
        })
    }

    #[instrument(name = "ledger.create_journal_for_account", skip(self, tx))]
    pub async fn create_journal_for_account(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: AccountId,
        account_name: String,
    ) -> Result<JournalId, BriaError> {
        let new_journal = NewJournal::builder()
            .id(Uuid::from(id))
            .description(format!("Journal for account '{}'", account_name))
            .name(account_name)
            .build()
            .expect("Couldn't build NewJournal");
        let id = self.inner.journals().create_in_tx(tx, new_journal).await?;
        Ok(id)
    }

    #[instrument(name = "ledger.create_ledger_accounts_for_wallet", skip(self, tx))]
    pub async fn create_ledger_accounts_for_wallet(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        wallet_id: WalletId,
        wallet_name: &str,
    ) -> Result<LedgerAccountId, BriaError> {
        let dust_account = NewLedgerAccount::builder()
            .name(format!("{}-dust", wallet_id))
            .code(format!("WALLET_{}_DUST", wallet_id))
            .description(format!("Dust account for wallet '{}'", wallet_name))
            .build()
            .expect("Couldn't build NewLedgerAccount");
        let dust_account_id = self.inner.accounts().create_in_tx(tx, dust_account).await?;
        let new_account = NewLedgerAccount::builder()
            .id(Uuid::from(wallet_id))
            .name(wallet_id.to_string())
            .code(format!("WALLET_{}", wallet_id))
            .description(format!("Account for wallet '{}'", wallet_name))
            .build()
            .expect("Couldn't build NewLedgerAccount");
        self.inner.accounts().create_in_tx(tx, new_account).await?;
        Ok(dust_account_id)
    }

    #[instrument(name = "ledger.incoming_utxo", skip(self, tx))]
    pub async fn incoming_utxo(
        &self,
        tx: Transaction<'_, Postgres>,
        params: IncomingUtxoParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, INCOMING_UTXO_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.confirmed_utxo", skip(self, tx))]
    pub async fn confirmed_utxo(
        &self,
        tx: Transaction<'_, Postgres>,
        params: ConfirmedUtxoParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, CONFIRMED_UTXO_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.queued_payout", skip(self, tx))]
    pub async fn queued_payout(
        &self,
        tx: Transaction<'_, Postgres>,
        params: QueuedPayoutParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, QUEUED_PAYOUT_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.get_balance")]
    pub async fn get_balance(
        &self,
        journal_id: JournalId,
        account_id: LedgerAccountId,
    ) -> Result<Option<LedgerAccountBalance>, BriaError> {
        let balance = self
            .inner
            .balances()
            .find(journal_id, account_id, self.btc)
            .await?;
        Ok(balance)
    }

    #[instrument(name = "ledger.onchain_income_account", skip_all)]
    async fn onchain_income_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(ONCHAIN_INCOME_CODE)
            .id(ONCHAIN_INCOMING_ID)
            .name(ONCHAIN_INCOME_CODE)
            .description("Account for settlement of onchain".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create ONCHAIN_INCOME");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(LedgerAccountId::from(ONCHAIN_INCOMING_ID)),
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }
}
