mod constants;
mod event;
mod templates;

use sqlx::{PgPool, Postgres, Transaction};
use sqlx_ledger::{
    account::NewAccount as NewLedgerAccount, balance::AccountBalance, event::*, journal::*,
    Currency, DebitOrCredit, JournalId, SqlxLedger, SqlxLedgerError,
};
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tracing::instrument;
use uuid::Uuid;

use std::collections::HashMap;

use crate::{account::balance::*, error::*, primitives::*, wallet::balance::*};
use constants::*;
pub use event::*;
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

        Self::effective_income_account(&inner).await?;
        Self::effective_at_rest_account(&inner).await?;
        Self::effective_outgoing_account(&inner).await?;

        templates::UtxoDetected::init(&inner).await?;
        templates::UtxoSettled::init(&inner).await?;
        templates::SpentUtxoSettled::init(&inner).await?;
        templates::SpendDetected::init(&inner).await?;
        templates::SpendSettled::init(&inner).await?;
        templates::PayoutQueued::init(&inner).await?;
        templates::BatchCreated::init(&inner).await?;
        templates::BatchSubmitted::init(&inner).await?;

        Ok(Self {
            inner,
            btc: "BTC".parse().unwrap(),
        })
    }

    pub async fn journal_events(
        &self,
        journal_id: JournalId,
        last_ledger_id: Option<SqlxLedgerEventId>,
    ) -> Result<impl Stream<Item = Result<JournalEvent, BriaError>>, BriaError> {
        let stream = BroadcastStream::new(
            self.inner
                .events(EventSubscriberOpts {
                    buffer: 100,
                    close_on_lag: true,
                    after_id: Some(last_ledger_id.unwrap_or(SqlxLedgerEventId::BEGIN)),
                })
                .await?
                .journal(journal_id)
                .await?,
        );
        Ok(stream.filter_map(|event| {
            match event
                .map_err(BriaError::from)
                .and_then(MaybeIgnored::try_from)
            {
                Ok(MaybeIgnored::Ignored) => None,
                Ok(MaybeIgnored::Event(e)) => Some(Ok(e)),
                Err(e) => Some(Err(e)),
            }
        }))
    }

    #[instrument(name = "ledger.utxo_detected", skip(self, tx))]
    pub async fn utxo_detected(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: UtxoDetectedParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, UTXO_DETECTED_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.utxo_settled", skip(self, tx))]
    pub async fn utxo_settled(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: UtxoSettledParams,
    ) -> Result<(), BriaError> {
        let (code, params) = if let Some(spent_tx) = params.meta.already_spent_tx_id {
            #[derive(serde::Deserialize)]
            struct ExtractAllocations {
                withdraw_from_effective_when_settled: HashMap<bitcoin::OutPoint, Satoshis>,
            }
            let txs = self
                .inner
                .transactions()
                .list_by_ids(std::iter::once(spent_tx))
                .await?;
            let outpoint = params.meta.outpoint;
            let mut params = sqlx_ledger::tx_template::TxParams::from(params);
            if let Some(tx) = txs.get(0) {
                if let Ok(Some(ExtractAllocations {
                    mut withdraw_from_effective_when_settled,
                })) = tx.metadata()
                {
                    let withdraw_from_effective_settled = withdraw_from_effective_when_settled
                        .remove(&outpoint)
                        .unwrap_or(Satoshis::ZERO);
                    params.insert(
                        "withdraw_from_effective_settled",
                        withdraw_from_effective_settled.to_btc(),
                    );
                }
            }
            (SPENT_UTXO_SETTLED_CODE, Some(params))
        } else {
            (
                UTXO_SETTLED_CODE,
                Some(sqlx_ledger::tx_template::TxParams::from(params)),
            )
        };
        self.inner
            .post_transaction_in_tx(tx, tx_id, code, params)
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.payout_queued", skip(self, tx))]
    pub async fn payout_queued(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: PayoutQueuedParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, PAYOUT_QUEUED_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.batch_created", skip(self, tx))]
    pub async fn batch_created(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: BatchCreatedParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, BATCH_CREATED_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.batch_submitted", skip(self, tx))]
    pub async fn batch_submitted(
        &self,
        tx: Transaction<'_, Postgres>,
        create_batch_tx_id: LedgerTransactionId,
        submit_tx_id: LedgerTransactionId,
        fees_to_encumber: Satoshis,
        ledger_account_ids: WalletLedgerAccountIds,
    ) -> Result<(), BriaError> {
        let txs = self
            .inner
            .transactions()
            .list_by_ids(std::iter::once(create_batch_tx_id))
            .await?;
        if let Some(BatchCreatedMeta {
            batch_info,
            tx_summary,
        }) = txs[0].metadata()?
        {
            let params = BatchSubmittedParams {
                journal_id: txs[0].journal_id,
                ledger_account_ids,
                meta: BatchSubmittedMeta {
                    batch_info,
                    encumbered_spending_fees: tx_summary
                        .change_utxos
                        .iter()
                        .map(|u| (u.outpoint, fees_to_encumber))
                        .collect(),
                    tx_summary,
                    withdraw_from_effective_when_settled: HashMap::new(),
                },
            };
            self.inner
                .post_transaction_in_tx(tx, submit_tx_id, BATCH_SUBMITTED_CODE, Some(params))
                .await?;
        }
        Ok(())
    }

    #[instrument(name = "ledger.spend_detected", skip(self, tx))]
    pub async fn spend_detected(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: SpendDetectedParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, SPEND_DETECTED_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.spend_settled", skip(self, tx))]
    #[allow(clippy::too_many_arguments)]
    pub async fn spend_settled(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        journal_id: JournalId,
        ledger_account_ids: WalletLedgerAccountIds,
        spend_detected_tx_id: LedgerTransactionId,
        confirmation_time: bitcoin::BlockTime,
        change_spent: bool,
    ) -> Result<(), BriaError> {
        #[derive(serde::Deserialize)]
        struct ExtractTxSummary {
            batch_info: Option<BatchWalletInfo>,
            tx_summary: WalletTransactionSummary,
        }
        let txs = self
            .inner
            .transactions()
            .list_by_ids(std::iter::once(spend_detected_tx_id))
            .await?;
        if let Some(ExtractTxSummary {
            tx_summary,
            batch_info,
        }) = txs[0].metadata()?
        {
            self.inner
                .post_transaction_in_tx(
                    tx,
                    tx_id,
                    SPEND_SETTLED_CODE,
                    Some(SpendSettledParams {
                        journal_id,
                        ledger_account_ids,
                        spend_detected_tx_id,
                        change_spent,
                        meta: SpendSettledMeta {
                            batch_info,
                            tx_summary,
                            confirmation_time,
                        },
                    }),
                )
                .await?;
        }
        Ok(())
    }

    #[instrument(name = "ledger.get_ledger_entries_for_txns", skip(self, tx_ids))]
    pub async fn sum_reserved_fees_in_txs(
        &self,
        tx_ids: HashMap<LedgerTransactionId, Vec<bitcoin::OutPoint>>,
    ) -> Result<Satoshis, BriaError> {
        let mut reserved_fees = Satoshis::from(0);
        #[derive(serde::Deserialize)]
        struct ExtractSpendingFees {
            #[serde(default)]
            encumbered_spending_fees: EncumberedSpendingFees,
        }
        let txs = self.inner.transactions().list_by_ids(tx_ids.keys()).await?;
        for tx in txs {
            if let Some(ExtractSpendingFees {
                encumbered_spending_fees,
            }) = tx.metadata()?
            {
                for outpoint in tx_ids.get(&tx.id).unwrap_or(&vec![]) {
                    reserved_fees += encumbered_spending_fees
                        .get(outpoint)
                        .copied()
                        .unwrap_or(Satoshis::ZERO);
                }
            }
        }
        Ok(reserved_fees)
    }

    #[instrument(name = "ledger.get_wallet_ledger_account_balances", skip(self))]
    pub async fn get_wallet_ledger_account_balances(
        &self,
        journal_id: JournalId,
        WalletLedgerAccountIds {
            onchain_incoming_id,
            onchain_at_rest_id,
            onchain_outgoing_id,
            effective_incoming_id,
            effective_at_rest_id,
            effective_outgoing_id,
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
                    effective_incoming_id,
                    effective_at_rest_id,
                    effective_outgoing_id,
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
            effective_incoming: balances
                .get_mut(&effective_incoming_id)
                .and_then(|b| b.remove(&self.btc)),
            effective_at_rest: balances
                .get_mut(&effective_at_rest_id)
                .and_then(|b| b.remove(&self.btc)),
            effective_outgoing: balances
                .get_mut(&effective_outgoing_id)
                .and_then(|b| b.remove(&self.btc)),
            fee: balances.get_mut(&fee_id).and_then(|b| b.remove(&self.btc)),
            dust: balances.get_mut(&dust_id).and_then(|b| b.remove(&self.btc)),
        })
    }

    #[instrument(name = "ledger.get_account_ledger_account_balances", skip(self))]
    pub async fn get_account_ledger_account_balances(
        &self,
        journal_id: JournalId,
    ) -> Result<AccountLedgerAccountBalances, BriaError> {
        let mut balances = self
            .inner
            .balances()
            .find_all(
                journal_id,
                [
                    sqlx_ledger::AccountId::from(ONCHAIN_UTXO_INCOMING_ID),
                    sqlx_ledger::AccountId::from(ONCHAIN_UTXO_AT_REST_ID),
                    sqlx_ledger::AccountId::from(ONCHAIN_UTXO_OUTGOING_ID),
                    sqlx_ledger::AccountId::from(EFFECTIVE_INCOMING_ID),
                    sqlx_ledger::AccountId::from(EFFECTIVE_AT_REST_ID),
                    sqlx_ledger::AccountId::from(EFFECTIVE_OUTGOING_ID),
                    sqlx_ledger::AccountId::from(ONCHAIN_FEE_ID),
                ],
            )
            .await?;
        Ok(AccountLedgerAccountBalances {
            onchain_incoming: balances
                .get_mut(&sqlx_ledger::AccountId::from(ONCHAIN_UTXO_INCOMING_ID))
                .and_then(|b| b.remove(&self.btc)),
            onchain_at_rest: balances
                .get_mut(&sqlx_ledger::AccountId::from(ONCHAIN_UTXO_AT_REST_ID))
                .and_then(|b| b.remove(&self.btc)),
            onchain_outgoing: balances
                .get_mut(&sqlx_ledger::AccountId::from(ONCHAIN_UTXO_OUTGOING_ID))
                .and_then(|b| b.remove(&self.btc)),
            effective_incoming: balances
                .get_mut(&sqlx_ledger::AccountId::from(EFFECTIVE_INCOMING_ID))
                .and_then(|b| b.remove(&self.btc)),
            effective_at_rest: balances
                .get_mut(&sqlx_ledger::AccountId::from(EFFECTIVE_AT_REST_ID))
                .and_then(|b| b.remove(&self.btc)),
            effective_outgoing: balances
                .get_mut(&sqlx_ledger::AccountId::from(EFFECTIVE_OUTGOING_ID))
                .and_then(|b| b.remove(&self.btc)),
            fee: balances
                .get_mut(&sqlx_ledger::AccountId::from(ONCHAIN_FEE_ID))
                .and_then(|b| b.remove(&self.btc)),
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
            .id(id)
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
            effective_incoming_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_EFFECTIVE_INCOMING"),
                    format!("{wallet_id}-effective-incoming"),
                    DebitOrCredit::Credit,
                )
                .await?,
            effective_at_rest_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_EFFECTIVE_AT_REST"),
                    format!("{wallet_id}-effective-at-rest"),
                    DebitOrCredit::Credit,
                )
                .await?,
            effective_outgoing_id: self
                .create_account_for_wallet(
                    tx,
                    wallet_id,
                    format!("WALLET_{wallet_id}_EFFECTIVE_OUTGOING"),
                    format!("{wallet_id}-effective-outgoing"),
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
            .build()
            .expect("Couldn't create onchain fee account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(LedgerAccountId::from(ONCHAIN_FEE_ID)),
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }

    #[instrument(name = "ledger.effective_income_account", skip_all)]
    async fn effective_income_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(EFFECTIVE_INCOMING_CODE)
            .id(EFFECTIVE_INCOMING_ID)
            .name(EFFECTIVE_INCOMING_CODE)
            .description("Account for effective incoming unconfirmed funds".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create effective incoming account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => {
                Ok(LedgerAccountId::from(EFFECTIVE_INCOMING_ID))
            }
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }

    #[instrument(name = "ledger.effective_at_rest_account", skip_all)]
    async fn effective_at_rest_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(EFFECTIVE_AT_REST_CODE)
            .id(EFFECTIVE_AT_REST_ID)
            .name(EFFECTIVE_AT_REST_CODE)
            .description("Account for settlement of effective funds".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create effective at rest account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => {
                Ok(LedgerAccountId::from(EFFECTIVE_AT_REST_ID))
            }
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }

    #[instrument(name = "ledger.effective_outgoing_account", skip_all)]
    async fn effective_outgoing_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(EFFECTIVE_OUTGOING_CODE)
            .id(EFFECTIVE_OUTGOING_ID)
            .name(EFFECTIVE_OUTGOING_CODE)
            .description("Account for outgoing effective funds".to_string())
            .normal_balance_type(DebitOrCredit::Debit)
            .build()
            .expect("Couldn't create effective  account");
        match ledger.accounts().create(new_account).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => {
                Ok(LedgerAccountId::from(EFFECTIVE_OUTGOING_ID))
            }
            Err(e) => Err(e.into()),
            Ok(id) => Ok(id),
        }
    }
}
