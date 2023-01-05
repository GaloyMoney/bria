mod config;

use sqlxmq::OwnedHandle;
use tracing::instrument;

pub use config::*;

use crate::{
    account::keys::*, batch::*, batch_group::*, error::*, job, ledger::*, payout::*, primitives::*,
    wallet::*, xpub::*,
};

pub struct App {
    _runner: OwnedHandle,
    keys: AccountApiKeys,
    xpubs: XPubs,
    wallets: Wallets,
    batch_groups: BatchGroups,
    payouts: Payouts,
    ledger: Ledger,
    pool: sqlx::PgPool,
    blockchain_cfg: BlockchainConfig,
}

impl App {
    pub async fn run(
        pool: sqlx::PgPool,
        blockchain_cfg: BlockchainConfig,
        wallets_cfg: WalletsConfig,
    ) -> Result<Self, BriaError> {
        let wallets = Wallets::new(&pool, blockchain_cfg.network);
        let batch_groups = BatchGroups::new(&pool);
        let batches = Batches::new(&pool);
        let payouts = Payouts::new(&pool);
        let ledger = Ledger::init(&pool).await?;
        let runner = job::start_job_runner(
            &pool,
            wallets.clone(),
            batch_groups.clone(),
            batches,
            payouts.clone(),
            ledger.clone(),
            wallets_cfg.sync_all_wallets_delay,
            wallets_cfg.process_all_batch_groups_delay,
            blockchain_cfg.clone(),
        )
        .await?;
        Self::spawn_sync_all_wallets(pool.clone(), wallets_cfg.sync_all_wallets_delay).await?;
        Self::spawn_process_all_batch_groups(
            pool.clone(),
            wallets_cfg.process_all_batch_groups_delay,
        )
        .await?;
        Ok(Self {
            keys: AccountApiKeys::new(&pool),
            xpubs: XPubs::new(&pool),
            wallets,
            batch_groups,
            payouts,
            pool,
            ledger,
            _runner: runner,
            blockchain_cfg,
        })
    }

    #[instrument(name = "app.authenticate", skip_all, err)]
    pub async fn authenticate(&self, key: &str) -> Result<AccountId, BriaError> {
        let key = self.keys.find_by_key(key).await?;
        Ok(key.account_id)
    }

    #[instrument(name = "app.import_xpub", skip(self), err)]
    pub async fn import_xpub(
        &self,
        account_id: AccountId,
        key_name: String,
        xpub: String,
        derivation: Option<String>,
    ) -> Result<XPubId, BriaError> {
        let value = XPub::try_from((xpub, derivation))?;
        let xpub = NewXPub::builder()
            .account_id(account_id)
            .key_name(key_name)
            .value(value)
            .build()
            .expect("Couldn't build xpub");
        let id = self.xpubs.persist(xpub).await?;
        Ok(id)
    }

    #[instrument(name = "app.set_signer_config", skip(self), err)]
    pub async fn set_signer_config(
        &self,
        account_id: AccountId,
        xpub_ref: String,
        config: SignerConfig,
    ) -> Result<(), BriaError> {
        let xpub = self.xpubs.find_from_ref(account_id, xpub_ref).await?;
        let new_signer = NewSigner::builder()
            .xpub_name(xpub.key_name)
            .config(config)
            .build()
            .expect("Couldn't build signer");
        self.xpubs
            .set_signer_for_xpub(account_id, new_signer)
            .await?;
        Ok(())
    }

    #[instrument(name = "app.create_wallet", skip(self), err)]
    pub async fn create_wallet(
        &self,
        account_id: AccountId,
        wallet_name: String,
        xpub_refs: Vec<String>,
    ) -> Result<WalletId, BriaError> {
        let mut xpubs = Vec::new();
        for xpub_ref in xpub_refs {
            xpubs.push(self.xpubs.find_from_ref(account_id, xpub_ref).await?);
        }

        if xpubs.len() != 1 {
            unimplemented!()
        }

        let wallet_id = WalletId::new();
        let mut tx = self.pool.begin().await?;
        let wallet_ledger_accounts = self
            .ledger
            .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &wallet_name)
            .await?;
        let new_wallet = NewWallet::builder()
            .id(wallet_id)
            .name(wallet_name.clone())
            .keychain(WpkhKeyChainConfig::new(
                xpubs.into_iter().next().expect("xpubs is empty").value,
            ))
            .ledger_accounts(wallet_ledger_accounts)
            .build()
            .expect("Couldn't build NewWallet");
        let wallet_id = self
            .wallets
            .create_in_tx(&mut tx, account_id, new_wallet)
            .await?;

        tx.commit().await?;

        Ok(wallet_id)
    }

    #[instrument(name = "app.get_wallet_balance", skip(self), err)]
    pub async fn get_wallet_balance(
        &self,
        account_id: AccountId,
        wallet_name: String,
    ) -> Result<WalletLedgerAccountBalances, BriaError> {
        let wallet = self.wallets.find_by_name(account_id, wallet_name).await?;

        let balances = WalletLedgerAccountBalances {
            incoming: self
                .ledger
                .get_balance(wallet.journal_id, wallet.ledger_accounts.incoming_id)
                .await?,
            at_rest: self
                .ledger
                .get_balance(wallet.journal_id, wallet.ledger_accounts.at_rest_id)
                .await?,
            fee: self
                .ledger
                .get_balance(wallet.journal_id, wallet.ledger_accounts.fee_id)
                .await?,
            outgoing: self
                .ledger
                .get_balance(wallet.journal_id, wallet.ledger_accounts.outgoing_id)
                .await?,
            dust: self
                .ledger
                .get_balance(wallet.journal_id, wallet.ledger_accounts.dust_id)
                .await?,
        };

        Ok(balances)
    }

    #[instrument(name = "app.new_address", skip(self), err)]
    pub async fn new_address(
        &self,
        account_id: AccountId,
        wallet_name: String,
    ) -> Result<String, BriaError> {
        let wallet = self.wallets.find_by_name(account_id, wallet_name).await?;
        let keychain_wallet = wallet.current_keychain_wallet(&self.pool);
        let addr = keychain_wallet.new_external_address().await?;
        Ok(addr.to_string())
    }

    #[instrument(name = "app.create_batch_group", skip(self), err)]
    pub async fn create_batch_group(
        &self,
        account_id: AccountId,
        batch_group_name: String,
    ) -> Result<BatchGroupId, BriaError> {
        let batch_group = NewBatchGroup::builder()
            .account_id(account_id)
            .name(batch_group_name)
            .build()
            .expect("Couldn't build NewBatchGroup");
        let batch_group_id = self.batch_groups.create(batch_group).await?;
        Ok(batch_group_id)
    }

    #[instrument(name = "app.queue_payout", skip(self), err)]
    #[allow(clippy::too_many_arguments)]
    pub async fn queue_payout(
        &self,
        account_id: AccountId,
        wallet_name: String,
        group_name: String,
        destination: PayoutDestination,
        sats: u64,
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<PayoutId, BriaError> {
        let wallet = self.wallets.find_by_name(account_id, wallet_name).await?;
        let group_id = self
            .batch_groups
            .find_by_name(account_id, group_name)
            .await?;
        let mut builder = NewPayout::builder();
        builder
            .wallet_id(wallet.id)
            .batch_group_id(group_id)
            .destination(destination.clone())
            .satoshis(sats)
            .metadata(metadata.clone());
        if let Some(external_id) = external_id.as_ref() {
            builder.external_id(external_id);
        }
        let new_payout = builder.build().expect("Couldn't build NewPayout");
        let mut tx = self.pool.begin().await?;
        let id = self
            .payouts
            .create_in_tx(&mut tx, account_id, new_payout)
            .await?;
        self.ledger
            .queued_payout(
                tx,
                QueuedPayoutParams {
                    journal_id: wallet.journal_id,
                    ledger_account_outgoing_id: wallet.ledger_accounts.outgoing_id,
                    external_id: external_id.unwrap_or_else(|| id.to_string()),
                    satoshis: sats,
                    meta: QueuedPayoutMeta {
                        payout_id: id,
                        batch_group_id: group_id,
                        wallet_id: wallet.id,
                        destination,
                        additional_meta: metadata,
                    },
                },
            )
            .await?;
        Ok(id)
    }

    #[instrument(name = "app.spawn_sync_all_wallets", skip_all, err)]
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

    #[instrument(name = "app.spawn_process_all_batch_groups", skip_all, err)]
    async fn spawn_process_all_batch_groups(
        pool: sqlx::PgPool,
        delay: std::time::Duration,
    ) -> Result<(), BriaError> {
        let _ = tokio::spawn(async move {
            loop {
                let _ =
                    job::spawn_process_all_batch_groups(&pool, std::time::Duration::from_secs(1))
                        .await;
                tokio::time::sleep(delay).await;
            }
        });
        Ok(())
    }
}
