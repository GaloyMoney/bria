mod constants;
mod templates;

use std::collections::HashMap;

use sqlx::{PgPool, Postgres, Transaction};
use sqlx_ledger::{
    account::NewAccount as NewLedgerAccount, balance::AccountBalance, entry::Entry as LedgerEntry,
    journal::*, Currency, DebitOrCredit, JournalId, SqlxLedger, SqlxLedgerError,
};
use tracing::instrument;
use uuid::Uuid;

use crate::{error::*, primitives::*, wallet::balance::*};
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
        Self::onchain_at_rest_account(&inner).await?;
        Self::onchain_outgoing_account(&inner).await?;
        Self::onchain_fee_account(&inner).await?;

        Self::logical_income_account(&inner).await?;
        Self::logical_at_rest_account(&inner).await?;
        Self::logical_outgoing_account(&inner).await?;

        templates::IncomingUtxo::init(&inner).await?;
        templates::OldIncomingUtxo::init(&inner).await?;
        templates::ConfirmedUtxo::init(&inner).await?;
        templates::QueuedPayout::init(&inner).await?;
        templates::CreateBatch::init(&inner).await?;

        Ok(Self {
            inner,
            btc: "BTC".parse().unwrap(),
        })
    }

    #[instrument(name = "ledger.incoming_utxo", skip(self, tx))]
    pub async fn incoming_utxo(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: IncomingUtxoParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, INCOMING_UTXO_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.incoming_utxo", skip(self, tx))]
    pub async fn old_incoming_utxo(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: OldIncomingUtxoParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, OLD_INCOMING_UTXO_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.confirmed_utxo", skip(self, tx))]
    pub async fn confirmed_utxo(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: ConfirmedUtxoParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, CONFIRMED_UTXO_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.queued_payout", skip(self, tx))]
    pub async fn queued_payout(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: QueuedPayoutParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, QUEUED_PAYOUT_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.create_batch", skip(self, tx))]
    pub async fn create_batch(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: CreateBatchParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, CREATE_BATCH_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.get_ledger_entries_for_txns", skip(self))]
    pub async fn get_ledger_entries_for_txns(
        &self,
        tx_ids: Vec<LedgerTransactionId>,
    ) -> Result<HashMap<LedgerTransactionId, Vec<LedgerEntry>>, BriaError> {
        let entries = self.inner.entries().list_by_transaction_ids(tx_ids).await?;
        Ok(entries)
    }

    #[instrument(name = "ledger.get_wallet_ledger_account_balances", skip(self))]
    pub async fn get_wallet_ledger_account_balances(
        &self,
        journal_id: JournalId,
        WalletLedgerAccountIds {
            onchain_incoming_id,
            onchain_at_rest_id,
            onchain_outgoing_id,
            logical_incoming_id,
            logical_at_rest_id,
            logical_outgoing_id,
            fee_id,
            dust_id,
        }: WalletLedgerAccountIds,
    ) -> Result<WalletLedgerAccountBalances, BriaError> {
        let mut balances = self
            .inner
            .balances()
            .find_all(
                journal_id,
                [
                    onchain_incoming_id,
                    onchain_at_rest_id,
                    onchain_outgoing_id,
                    logical_incoming_id,
                    logical_at_rest_id,
                    logical_outgoing_id,
                    fee_id,
                    dust_id,
                ],
            )
            .await?;
        Ok(WalletLedgerAccountBalances {
            onchain_incoming: balances
                .get_mut(&onchain_incoming_id)
                .and_then(|b| b.remove(&self.btc)),
            onchain_at_rest: balances
                .get_mut(&onchain_at_rest_id)
                .and_then(|b| b.remove(&self.btc)),
            onchain_outgoing: balances
                .get_mut(&onchain_outgoing_id)
                .and_then(|b| b.remove(&self.btc)),
            logical_incoming: balances
                .get_mut(&logical_incoming_id)
                .and_then(|b| b.remove(&self.btc)),
            logical_at_rest: balances
                .get_mut(&logical_at_rest_id)
                .and_then(|b| b.remove(&self.btc)),
            logical_outgoing: balances
                .get_mut(&logical_outgoing_id)
                .and_then(|b| b.remove(&self.btc)),
            fee: balances.get_mut(&fee_id).and_then(|b| b.remove(&self.btc)),
            dust: balances.get_mut(&dust_id).and_then(|b| b.remove(&self.btc)),
        })
    }

    #[instrument(name = "ledger.get_ledger_account_balance")]
    pub async fn get_ledger_account_balance(
        &self,
        journal_id: JournalId,
        account_id: LedgerAccountId,
    ) -> Result<Option<AccountBalance>, BriaError> {
        Ok(self
            .inner
            .balances()
            .find(journal_id, account_id, self.btc)
            .await?)
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
            .description(format!("Journal for account '{account_name}'"))
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
    ) -> Result<WalletLedgerAccountIds, BriaError> {
        let account_ids = WalletLedgerAccountIds {
            onchain_incoming_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_UTXO_INCOMING"),
                    format!("{wallet_id}-utxo-incoming"),
                    DebitOrCredit::Credit,
                )
                .await?,
            onchain_at_rest_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_UTXO_AT_REST"),
                    format!("{wallet_id}-utxo-at-rest"),
                    DebitOrCredit::Credit,
                )
                .await?,
            onchain_outgoing_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_UTXO_OUTGOING"),
                    format!("{wallet_id}-utxo-outgoing"),
                    DebitOrCredit::Credit,
                )
                .await?,
            logical_incoming_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_LOGICAL_INCOMING"),
                    format!("{wallet_id}-logical-incoming"),
                    DebitOrCredit::Credit,
                )
                .await?,
            logical_at_rest_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_LOGICAL_AT_REST"),
                    format!("{wallet_id}-logical-at-rest"),
                    DebitOrCredit::Credit,
                )
                .await?,
            logical_outgoing_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_LOGICAL_OUTGOING"),
                    format!("{wallet_id}-logical-outgoing"),
                    DebitOrCredit::Credit,
                )
                .await?,
            fee_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_ONCHAIN_FEE"),
                    format!("{wallet_id}-onchain-fee"),
                    DebitOrCredit::Debit,
                )
                .await?,
            dust_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_DUST"),
                    format!("{wallet_id}-dust"),
                    DebitOrCredit::Credit,
                )
                .await?,
        };
        Ok(account_ids)
    }

    #[instrument(name = "ledger.create_account_for_wallet", skip(self, tx))]
    async fn create_account_for_wallet(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        wallet_id: WalletId,
        wallet_code: String,
        wallet_name: String,
        balance_type: DebitOrCredit,
    ) -> Result<LedgerAccountId, BriaError> {
        let account = NewLedgerAccount::builder()
            .id(Uuid::new_v4())
            .name(&wallet_name)
            .code(wallet_code)
            .description(format!("Account for wallet '{}'", &wallet_id))
            .normal_balance_type(balance_type)
            .build()
            .expect("Couldn't build NewLedgerAccount");
        let account_id = self.inner.accounts().create_in_tx(tx, account).await?;
        Ok(account_id)
    }

    #[instrument(name = "ledger.onchain_income_account", skip_all)]
    async fn onchain_income_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(ONCHAIN_UTXO_INCOMING_CODE)
            .id(ONCHAIN_UTXO_INCOMING_ID)
            .name(ONCHAIN_UTXO_INCOMING_CODE)
            .description("Account for onchain incoming unconfirmed funds".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create onchain incoming account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => {
                Ok(LedgerAccountId::from(ONCHAIN_UTXO_INCOMING_ID))
            }
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }

    #[instrument(name = "ledger.onchain_at_rest_account", skip_all)]
    async fn onchain_at_rest_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(ONCHAIN_UTXO_AT_REST_CODE)
            .id(ONCHAIN_UTXO_AT_REST_ID)
            .name(ONCHAIN_UTXO_AT_REST_CODE)
            .description("Account for settlement of onchain funds".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create onchain at rest account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => {
                Ok(LedgerAccountId::from(ONCHAIN_UTXO_AT_REST_ID))
            }
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }

    #[instrument(name = "ledger.onchain_outgoing_account", skip_all)]
    async fn onchain_outgoing_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(ONCHAIN_UTXO_OUTGOING_CODE)
            .id(ONCHAIN_UTXO_OUTGOING_ID)
            .name(ONCHAIN_UTXO_OUTGOING_CODE)
            .description("Account for outgoing onchain funds".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create onchain  account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => {
                Ok(LedgerAccountId::from(ONCHAIN_UTXO_OUTGOING_ID))
            }
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }

    #[instrument(name = "ledger.onchain_fee_account", skip_all)]
    async fn onchain_fee_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(ONCHAIN_FEE_CODE)
            .id(ONCHAIN_FEE_ID)
            .name(ONCHAIN_FEE_CODE)
            .description("Account for provisioning of onchain fees".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create onchain fee account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(LedgerAccountId::from(ONCHAIN_FEE_ID)),
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }

    #[instrument(name = "ledger.logical_income_account", skip_all)]
    async fn logical_income_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(LOGICAL_INCOMING_CODE)
            .id(LOGICAL_INCOMING_ID)
            .name(LOGICAL_INCOMING_CODE)
            .description("Account for logical incoming unconfirmed funds".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create logical incoming account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(LedgerAccountId::from(LOGICAL_INCOMING_ID)),
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }

    #[instrument(name = "ledger.logical_at_rest_account", skip_all)]
    async fn logical_at_rest_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(LOGICAL_AT_REST_CODE)
            .id(LOGICAL_AT_REST_ID)
            .name(LOGICAL_AT_REST_CODE)
            .description("Account for settlement of logical funds".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create logical at rest account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(LedgerAccountId::from(LOGICAL_AT_REST_ID)),
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }

    #[instrument(name = "ledger.logical_outgoing_account", skip_all)]
    async fn logical_outgoing_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(LOGICAL_OUTGOING_CODE)
            .id(LOGICAL_OUTGOING_ID)
            .name(LOGICAL_OUTGOING_CODE)
            .description("Account for outgoing logical funds".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create logical  account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(LedgerAccountId::from(LOGICAL_OUTGOING_ID)),
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }
}
