mod params;

use sqlx::{PgPool, Postgres, Transaction};
use sqlx_ledger::{
    account::NewAccount as NewLedgerAccount, balance::AccountBalance as LedgerAccountBalance,
    journal::*, tx_template::*, AccountId as LedgerAccountId, Currency, DebitOrCredit, JournalId,
    SqlxLedger, SqlxLedgerError,
};
use uuid::{uuid, Uuid};

use crate::{error::*, primitives::*};
pub use params::*;

const ONCHAIN_INCOMING_CODE: &str = "ONCHAIN_INCOMING";
const ONCHAIN_INCOMING_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
const PENDING_ONCHAIN_CREDIT_CODE: &str = "PENDING_ONCHAIN_CREDIT";

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
        let inner = SqlxLedger::new(&pool);
        Self::onchain_income_account(&inner).await?;
        Self::incoming_utxo_template(&inner).await?;
        Ok(Self {
            inner,
            btc: "BTC".parse().unwrap(),
        })
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

    pub async fn pending_income(
        &self,
        tx: Transaction<'_, Postgres>,
        params: PendingOnchainIncomeParams,
    ) -> Result<(), BriaError> {
        self.inner
            .post_transaction_in_tx(tx, PENDING_ONCHAIN_CREDIT_CODE, Some(params))
            .await?;
        Ok(())
    }

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

    async fn onchain_income_account(ledger: &SqlxLedger) -> Result<LedgerAccountId, BriaError> {
        let new_account = NewLedgerAccount::builder()
            .code(ONCHAIN_INCOMING_CODE)
            .id(ONCHAIN_INCOMING_ID)
            .name(ONCHAIN_INCOMING_CODE)
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

    async fn incoming_utxo_template(ledger: &SqlxLedger) -> Result<(), BriaError> {
        let tx_input = TxInput::builder()
            .journal_id("params.journal_id")
            .effective("params.effective")
            .external_id("params.external_id")
            .metadata("params.meta")
            .description("'Income from onchain transaction'")
            .build()
            .expect("Couldn't build TxInput");
        let entries = vec![
            EntryInput::builder()
                .entry_type("'ONCHAIN_DR'")
                .currency("'BTC'")
                .account_id(format!("uuid('{}')", ONCHAIN_INCOMING_ID))
                .direction("DEBIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build ONCHAIN_DEBIT entry"),
            EntryInput::builder()
                .entry_type("'ONCHAIN_CR'")
                .currency("'BTC'")
                .account_id("params.recipient_account_id")
                .direction("CREDIT")
                .layer("PENDING")
                .units("params.amount")
                .build()
                .expect("Couldn't build ONCHAIN_DEBIT entry"),
        ];

        let params = PendingOnchainIncomeParams::defs();
        let template = NewTxTemplate::builder()
            .code(PENDING_ONCHAIN_CREDIT_CODE)
            .tx_input(tx_input)
            .entries(entries)
            .params(params)
            .build()
            .expect("Couldn't build PENDING_ONCHAIN_CREDIT_CODE");
        match ledger.tx_templates().create(template).await {
            Err(SqlxLedgerError::DuplicateKey(_)) => Ok(()),
            Err(e) => Err(e.into()),
            Ok(_) => Ok(()),
        }
    }
}
