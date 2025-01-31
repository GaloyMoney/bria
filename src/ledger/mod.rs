mod constants;
pub mod error;
mod event;
mod templates;
mod wallet_accounts;

use sqlx::{PgPool, Postgres, Transaction};
use sqlx_ledger::{
    account::NewAccount as NewLedgerAccount, event::*, journal::*, Currency, DebitOrCredit,
    JournalId, SqlxLedger, SqlxLedgerError,
};
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};
use tracing::instrument;

use rust_decimal::Decimal;
use std::collections::HashMap;

use crate::{account::balance::*, primitives::*};
use constants::*;
pub use error::LedgerError;
pub use event::*;
pub use templates::*;
pub use wallet_accounts::*;

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

    pub async fn init(pool: &PgPool) -> Result<Self, LedgerError> {
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
        templates::UtxoDropped::init(&inner).await?;
        templates::SpentUtxoSettled::init(&inner).await?;
        templates::SpendDetected::init(&inner).await?;
        templates::SpendSettled::init(&inner).await?;
        templates::PayoutSubmitted::init(&inner).await?;
        templates::PayoutCancelled::init(&inner).await?;
        if templates::BatchCreated::init(&inner).await? {
            templates::fix::legacy_batch_created(&inner).await?;
        }
        templates::BatchDropped::init(&inner).await?;
        templates::BatchBroadcast::init(&inner).await?;

        Ok(Self {
            inner,
            btc: "BTC".parse().unwrap(),
        })
    }

    pub async fn journal_events(
        &self,
        journal_id: JournalId,
        last_ledger_id: Option<SqlxLedgerEventId>,
    ) -> Result<impl Stream<Item = Result<JournalEvent, LedgerError>>, LedgerError> {
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
                .map_err(LedgerError::from)
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
    ) -> Result<(), LedgerError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, UTXO_DETECTED_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.utxo_dropped", skip(self, tx), err)]
    pub async fn utxo_dropped(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        detected_txn_id: LedgerTransactionId,
    ) -> Result<(), LedgerError> {
        let txs = self
            .inner
            .transactions()
            .list_by_ids(std::iter::once(detected_txn_id))
            .await?;

        let txn = txs.first().ok_or(LedgerError::TransactionNotFound)?;

        let UtxoDetectedMeta {
            account_id,
            wallet_id,
            keychain_id,
            outpoint,
            satoshis,
            address,
            encumbered_spending_fees,
            confirmation_time,
        } = txn
            .metadata()
            .map_err(LedgerError::MismatchedTxMetadata)?
            .ok_or(LedgerError::MissingTxMetadata)?;
        let entries = self
            .inner
            .entries()
            .list_by_transaction_ids(std::iter::once(detected_txn_id))
            .await?;

        let mut onchain_incoming_account_id = None;
        let mut effective_incoming_account_id = None;
        let mut onchain_fee_account_id = None;

        for entry in entries.into_values().flatten() {
            match entry.entry_type.as_str() {
                "UTXO_DETECTED_UTX_IN_PEN_CR" => {
                    onchain_incoming_account_id = Some(entry.account_id)
                }
                "UTXO_DETECTED_LOG_IN_PEN_CR" => {
                    effective_incoming_account_id = Some(entry.account_id)
                }
                "UTXO_DETECTED_FR_ENC_DR" => onchain_fee_account_id = Some(entry.account_id),
                _ => {}
            }
        }
        let onchain_incoming_account_id = onchain_incoming_account_id.ok_or(
            LedgerError::ExpectedEntryNotFoundInTx("Onchain incoming account ID not found"),
        )?;
        let effective_incoming_account_id = effective_incoming_account_id.ok_or(
            LedgerError::ExpectedEntryNotFoundInTx("Effective incoming account ID not found"),
        )?;
        let onchain_fee_account_id = onchain_fee_account_id.ok_or(
            LedgerError::ExpectedEntryNotFoundInTx("Onchain fee account ID not found"),
        )?;

        let params = UtxoDroppedParams {
            journal_id: txn.journal_id,
            onchain_incoming_account_id,
            effective_incoming_account_id,
            onchain_fee_account_id,
            meta: UtxoDroppedMeta {
                account_id,
                wallet_id,
                keychain_id,
                outpoint,
                satoshis,
                address,
                encumbered_spending_fees,
                confirmation_time,
                detected_txn_id,
            },
        };

        self.inner
            .post_transaction_in_tx(tx, tx_id, UTXO_DROPPED_CODE, Some(params))
            .await?;

        Ok(())
    }

    #[instrument(name = "ledger.utxo_settled", skip(self, tx))]
    pub async fn utxo_settled(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: UtxoSettledParams,
    ) -> Result<(), LedgerError> {
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
            if let Some(tx) = txs.first() {
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

    #[instrument(name = "ledger.payout_submitted", skip(self, tx))]
    pub async fn payout_submitted(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: impl Into<LedgerTransactionId> + std::fmt::Debug,
        params: PayoutSubmittedParams,
    ) -> Result<(), LedgerError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id.into(), PAYOUT_SUBMITTED_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.payout_cancelled", skip(self, tx))]
    pub async fn payout_cancelled(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        payout_submitted_tx_id: impl Into<LedgerTransactionId> + std::fmt::Debug,
    ) -> Result<(), LedgerError> {
        let payout_submitted_tx_id = payout_submitted_tx_id.into();
        let txs = self
            .inner
            .transactions()
            .list_by_ids(std::iter::once(payout_submitted_tx_id))
            .await?;
        let txn = txs.first().ok_or(LedgerError::TransactionNotFound)?;
        let PayoutSubmittedMeta {
            payout_id,
            account_id,
            wallet_id,
            profile_id,
            payout_queue_id,
            satoshis,
            destination,
        } = txn.metadata()?.ok_or(LedgerError::MissingTxMetadata)?;
        let entries = self
            .inner
            .entries()
            .list_by_transaction_ids(std::iter::once(payout_submitted_tx_id))
            .await?;
        let effective_outgoing_account_id = entries
            .into_values()
            .flatten()
            .find_map(|entry| match entry.entry_type.as_str() {
                "PAYOUT_SUBMITTED_LOG_OUT_ENC_CR" => Some(entry.account_id),
                _ => None,
            })
            .ok_or(LedgerError::ExpectedEntryNotFoundInTx(
                "Effective outgoing account ID not found",
            ))?;
        let params = PayoutCancelledParams {
            journal_id: txn.journal_id,
            effective_outgoing_account_id,
            payout_submitted_tx_id,
            meta: PayoutCancelledMeta {
                payout_id,
                account_id,
                wallet_id,
                profile_id,
                payout_queue_id,
                satoshis,
                destination,
            },
        };
        self.inner
            .post_transaction_in_tx(tx, tx_id, PAYOUT_CANCELLED_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.batch_created", skip(self, tx))]
    pub async fn batch_created(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        params: BatchCreatedParams,
    ) -> Result<(), LedgerError> {
        self.inner
            .post_transaction_in_tx(tx, tx_id, BATCH_CREATED_CODE, Some(params))
            .await?;
        Ok(())
    }

    #[instrument(name = "ledger.batch_dropped", skip(self, tx))]
    pub async fn batch_dropped(
        &self,
        tx: Transaction<'_, Postgres>,
        tx_id: LedgerTransactionId,
        created_batch_tx_id: LedgerTransactionId,
    ) -> Result<(), LedgerError> {
        let txs = self
            .inner
            .transactions()
            .list_by_ids(std::iter::once(created_batch_tx_id))
            .await?;
        let txn = txs.first().ok_or(LedgerError::TransactionNotFound)?;

        let BatchCreatedMeta {
            batch_info,
            tx_summary,
        } = txn.metadata()?.ok_or(LedgerError::MissingTxMetadata)?;

        let entries = self
            .inner
            .entries()
            .list_by_transaction_ids(std::iter::once(created_batch_tx_id))
            .await?;

        let mut ledger_account_ids = WalletLedgerAccountIds::default();
        let mut encumbered_fees: Satoshis = Satoshis::from_btc(Decimal::ZERO);
        for entry in entries.into_values().flatten() {
            match entry.entry_type.as_str() {
                "BATCH_CREATED_LOG_OUT_ENC_DR" => {
                    ledger_account_ids.effective_outgoing_id = entry.account_id;
                }
                "BATCH_CREATED_LOG_SET_DR" => {
                    ledger_account_ids.effective_at_rest_id = entry.account_id;
                }
                "BATCH_CREATED_FR_ENC_CR" => {
                    ledger_account_ids.fee_id = entry.account_id;
                    encumbered_fees = Satoshis::from_btc(entry.units);
                }
                "BATCH_CREATED_UTX_OUT_PEN_CR" => {
                    ledger_account_ids.onchain_outgoing_id = entry.account_id;
                }
                "BATCH_CREATED_CHG_ENC_CR" => {
                    ledger_account_ids.onchain_incoming_id = entry.account_id;
                }
                "BATCH_CREATED_UTX_SET_DR" => {
                    ledger_account_ids.onchain_at_rest_id = entry.account_id;
                }
                _ => {}
            }
        }

        let params = BatchDroppedParams {
            journal_id: txn.journal_id,
            ledger_account_ids,
            encumbered_fees,
            meta: BatchDroppedMeta {
                batch_info,
                tx_summary,
                created_txn_id: created_batch_tx_id,
            },
        };

        self.inner
            .post_transaction_in_tx(tx, tx_id, BATCH_DROPPED_CODE, Some(params))
            .await?;

        Ok(())
    }

    #[instrument(name = "ledger.batch_broadcast", skip(self, tx))]
    pub async fn batch_broadcast(
        &self,
        tx: Transaction<'_, Postgres>,
        create_batch_tx_id: LedgerTransactionId,
        submit_tx_id: LedgerTransactionId,
        fees_to_encumber: Satoshis,
        ledger_account_ids: WalletLedgerAccountIds,
    ) -> Result<(), LedgerError> {
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
            let params = BatchBroadcastParams {
                journal_id: txs[0].journal_id,
                ledger_account_ids,
                meta: BatchBroadcastMeta {
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
                .post_transaction_in_tx(tx, submit_tx_id, BATCH_BROADCAST_CODE, Some(params))
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
    ) -> Result<(), LedgerError> {
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
    ) -> Result<(), LedgerError> {
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
    ) -> Result<Satoshis, LedgerError> {
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
    ) -> Result<WalletLedgerAccountBalances, LedgerError> {
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
    ) -> Result<AccountLedgerAccountBalances, LedgerError> {
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

    #[instrument(name = "ledger.create_journal_for_account", skip(self, tx))]
    pub async fn create_journal_for_account(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: AccountId,
        account_name: String,
    ) -> Result<JournalId, LedgerError> {
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
        ids: impl Into<WalletLedgerAccountIds> + std::fmt::Debug,
    ) -> Result<WalletLedgerAccountIds, LedgerError> {
        let wallet_ledger_account_ids = ids.into();
        let prefix = wallet_ledger_account_ids.get_wallet_id_prefix();
        let account_ids = WalletLedgerAccountIds {
            onchain_incoming_id: self
                .create_account_for_wallet(
                    tx,
                    &prefix,
                    wallet_ledger_account_ids.onchain_incoming_id,
                    format!("WALLET_{prefix}_UTXO_INCOMING"),
                    format!("{prefix}-utxo-incoming"),
                    DebitOrCredit::Credit,
                )
                .await?,
            onchain_at_rest_id: self
                .create_account_for_wallet(
                    tx,
                    &prefix,
                    wallet_ledger_account_ids.onchain_at_rest_id,
                    format!("WALLET_{prefix}_UTXO_AT_REST"),
                    format!("{prefix}-utxo-at-rest"),
                    DebitOrCredit::Credit,
                )
                .await?,
            onchain_outgoing_id: self
                .create_account_for_wallet(
                    tx,
                    &prefix,
                    wallet_ledger_account_ids.onchain_outgoing_id,
                    format!("WALLET_{prefix}_UTXO_OUTGOING"),
                    format!("{prefix}-utxo-outgoing"),
                    DebitOrCredit::Credit,
                )
                .await?,
            effective_incoming_id: self
                .create_account_for_wallet(
                    tx,
                    &prefix,
                    wallet_ledger_account_ids.effective_incoming_id,
                    format!("WALLET_{prefix}_EFFECTIVE_INCOMING"),
                    format!("{prefix}-effective-incoming"),
                    DebitOrCredit::Credit,
                )
                .await?,
            effective_at_rest_id: self
                .create_account_for_wallet(
                    tx,
                    &prefix,
                    wallet_ledger_account_ids.effective_at_rest_id,
                    format!("WALLET_{prefix}_EFFECTIVE_AT_REST"),
                    format!("{prefix}-effective-at-rest"),
                    DebitOrCredit::Credit,
                )
                .await?,
            effective_outgoing_id: self
                .create_account_for_wallet(
                    tx,
                    &prefix,
                    wallet_ledger_account_ids.effective_outgoing_id,
                    format!("WALLET_{prefix}_EFFECTIVE_OUTGOING"),
                    format!("{prefix}-effective-outgoing"),
                    DebitOrCredit::Credit,
                )
                .await?,
            fee_id: self
                .create_account_for_wallet(
                    tx,
                    &prefix,
                    wallet_ledger_account_ids.fee_id,
                    format!("WALLET_{prefix}_ONCHAIN_FEE"),
                    format!("{prefix}-onchain-fee"),
                    DebitOrCredit::Debit,
                )
                .await?,
            dust_id: self
                .create_account_for_wallet(
                    tx,
                    &prefix,
                    wallet_ledger_account_ids.dust_id,
                    format!("WALLET_{prefix}_DUST"),
                    format!("{prefix}-dust"),
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
        wallet_id_prefix: &str,
        account_id: LedgerAccountId,
        wallet_code: String,
        wallet_name: String,
        balance_type: DebitOrCredit,
    ) -> Result<LedgerAccountId, LedgerError> {
        let account = NewLedgerAccount::builder()
            .id(account_id)
            .name(&wallet_name)
            .code(wallet_code)
            .description(format!("Account for wallet '{}'", wallet_id_prefix))
            .normal_balance_type(balance_type)
            .build()
            .expect("Couldn't build NewLedgerAccount");
        let account_id = self.inner.accounts().create_in_tx(tx, account).await?;
        Ok(account_id)
    }

    #[instrument(name = "ledger.onchain_income_account", skip_all)]
    async fn onchain_income_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, LedgerError> {
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
    async fn onchain_at_rest_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, LedgerError> {
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
    async fn onchain_outgoing_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, LedgerError> {
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
    async fn onchain_fee_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, LedgerError> {
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
    async fn effective_income_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, LedgerError> {
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
    async fn effective_at_rest_account(
        ledger: &SqlxLedger,
    ) -> Result<LedgerAccountId, LedgerError> {
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
    async fn effective_outgoing_account(
        ledger: &SqlxLedger,
    ) -> Result<LedgerAccountId, LedgerError> {
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
