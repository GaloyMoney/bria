mod config;

use sqlxmq::OwnedHandle;
use tracing::instrument;

pub use config::*;

use crate::{
    batch::*,
    batch_group::*,
    error::*,
    job,
    ledger::*,
    payout::*,
    primitives::*,
    profile::*,
    utxo::*,
    wallet::{balance::*, *},
    xpub::*,
};

#[allow(dead_code)]
pub struct App {
    _runner: OwnedHandle,
    profiles: Profiles,
    xpubs: XPubs,
    wallets: Wallets,
    batch_groups: BatchGroups,
    payouts: Payouts,
    ledger: Ledger,
    utxos: Utxos,
    pool: sqlx::PgPool,
    blockchain_cfg: BlockchainConfig,
}

impl App {
    pub async fn run(
        pool: sqlx::PgPool,
        migrate_on_start: bool,
        blockchain_cfg: BlockchainConfig,
        wallets_cfg: WalletsConfig,
    ) -> Result<Self, BriaError> {
        if migrate_on_start {
            sqlx::migrate!().run(&pool).await?;
        }
        let wallets = Wallets::new(&pool, blockchain_cfg.network);
        let batch_groups = BatchGroups::new(&pool);
        let batches = Batches::new(&pool);
        let payouts = Payouts::new(&pool);
        let ledger = Ledger::init(&pool).await?;
        let utxos = Utxos::new(&pool);
        let runner = job::start_job_runner(
            &pool,
            wallets.clone(),
            batch_groups.clone(),
            batches,
            payouts.clone(),
            ledger.clone(),
            utxos.clone(),
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
            profiles: Profiles::new(&pool),
            xpubs: XPubs::new(&pool),
            wallets,
            batch_groups,
            payouts,
            pool,
            ledger,
            utxos,
            _runner: runner,
            blockchain_cfg,
        })
    }

    #[instrument(name = "app.authenticate", skip_all, err)]
    pub async fn authenticate(&self, key: &str) -> Result<Profile, BriaError> {
        let profile = self.profiles.find_by_key(key).await?;
        Ok(profile)
    }

    #[instrument(name = "app.create_profile", skip(self), err)]
    pub async fn create_profile(
        &self,
        profile: Profile,
        name: String,
    ) -> Result<Profile, BriaError> {
        let mut tx = self.pool.begin().await?;
        let new_profile = self
            .profiles
            .create_in_tx(&mut tx, profile.account_id, name)
            .await?;
        tx.commit().await?;
        Ok(new_profile)
    }

    #[instrument(name = "app.list_profiles", skip(self), err)]
    pub async fn list_profiles(&self, profile: Profile) -> Result<Vec<Profile>, BriaError> {
        let profiles = self.profiles.list_for_account(profile.account_id).await?;
        Ok(profiles)
    }

    #[instrument(name = "app.create_profile_api_key", skip(self), err)]
    pub async fn create_profile_api_key(
        &self,
        profile: Profile,
        profile_name: String,
    ) -> Result<ProfileApiKey, BriaError> {
        let found_profile = self
            .profiles
            .find_by_name(profile.account_id, profile_name)
            .await?;
        let mut tx = self.pool.begin().await?;
        let key = self
            .profiles
            .create_key_for_profile_in_tx(&mut tx, found_profile)
            .await?;
        tx.commit().await?;
        Ok(key)
    }

    #[instrument(name = "app.import_xpub", skip(self), err)]
    pub async fn import_xpub(
        &self,
        profile: Profile,
        key_name: String,
        xpub: String,
        derivation: Option<String>,
    ) -> Result<XPubId, BriaError> {
        let value = XPub::try_from((xpub, derivation))?;
        let xpub = NewXPub::builder()
            .account_id(profile.account_id)
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
        profile: Profile,
        xpub_ref: String,
        config: SignerConfig,
    ) -> Result<(), BriaError> {
        let xpub = self
            .xpubs
            .find_from_ref(profile.account_id, xpub_ref)
            .await?;
        let new_signer = NewSigner::builder()
            .xpub_name(xpub.key_name)
            .config(config)
            .build()
            .expect("Couldn't build signer");
        self.xpubs
            .set_signer_for_xpub(profile.account_id, new_signer)
            .await?;
        Ok(())
    }

    #[instrument(name = "app.create_wallet", skip(self), err)]
    pub async fn create_wallet(
        &self,
        profile: Profile,
        wallet_name: String,
        xpub_refs: Vec<String>,
    ) -> Result<WalletId, BriaError> {
        let mut xpubs = Vec::new();
        for xpub_ref in xpub_refs {
            xpubs.push(
                self.xpubs
                    .find_from_ref(profile.account_id, xpub_ref)
                    .await?,
            );
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
            .ledger_account_ids(wallet_ledger_accounts)
            .build()
            .expect("Couldn't build NewWallet");
        let wallet_id = self
            .wallets
            .create_in_tx(&mut tx, profile.account_id, new_wallet)
            .await?;

        tx.commit().await?;

        Ok(wallet_id)
    }

    #[instrument(name = "app.get_wallet_balance_summary", skip(self), err)]
    pub async fn get_wallet_balance_summary(
        &self,
        profile: Profile,
        wallet_name: String,
    ) -> Result<WalletBalanceSummary, BriaError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        let wallet_ledger_account_balances = self
            .ledger
            .get_wallet_ledger_account_balances(wallet.journal_id, wallet.ledger_account_ids)
            .await?;
        let summary = WalletBalanceSummary::from(wallet_ledger_account_balances);

        Ok(summary)
    }

    #[instrument(name = "app.new_address", skip(self), err)]
    pub async fn new_address(
        &self,
        profile: Profile,
        wallet_name: String,
    ) -> Result<String, BriaError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        let keychain_wallet = wallet.current_keychain_wallet(&self.pool);
        let addr = keychain_wallet.new_external_address().await?;
        Ok(addr.to_string())
    }

    #[instrument(name = "app.list_utxos", skip(self), err)]
    pub async fn list_utxos(
        &self,
        profile: Profile,
        wallet_name: String,
    ) -> Result<(WalletId, Vec<KeychainUtxos>), BriaError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        let mut utxos = self
            .utxos
            .find_keychain_utxos(wallet.keychain_ids())
            .await?;
        let ordered_utxos = wallet
            .keychain_ids()
            .filter_map(|keychain_id| utxos.remove(&keychain_id))
            .collect();
        Ok((wallet.id, ordered_utxos))
    }

    #[instrument(name = "app.create_batch_group", skip(self), err)]
    pub async fn create_batch_group(
        &self,
        profile: Profile,
        batch_group_name: String,
        description: Option<String>,
        config: Option<BatchGroupConfig>,
    ) -> Result<BatchGroupId, BriaError> {
        let mut builder = NewBatchGroup::builder();
        builder
            .account_id(profile.account_id)
            .name(batch_group_name)
            .description(description);
        if let Some(config) = config {
            builder.config(config);
        }
        let batch_group = builder.build().expect("Couldn't build NewBatchGroup");
        let batch_group_id = self.batch_groups.create(batch_group).await?;
        Ok(batch_group_id)
    }

    #[instrument(name = "app.queue_payout", skip(self), err)]
    #[allow(clippy::too_many_arguments)]
    pub async fn queue_payout(
        &self,
        profile: Profile,
        wallet_name: String,
        group_name: String,
        destination: PayoutDestination,
        sats: Satoshis,
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<PayoutId, BriaError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        let group_id = self
            .batch_groups
            .find_by_name(profile.account_id, group_name)
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
            .create_in_tx(&mut tx, profile.account_id, new_payout)
            .await?;
        self.ledger
            .queued_payout(
                tx,
                LedgerTransactionId::from(uuid::Uuid::from(id)),
                QueuedPayoutParams {
                    journal_id: wallet.journal_id,
                    logical_outgoing_account_id: wallet.ledger_account_ids.logical_outgoing_id,
                    external_id: external_id.unwrap_or_else(|| id.to_string()),
                    payout_satoshis: sats,
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

    #[instrument(name = "app.list_payouts", skip_all, err)]
    pub async fn list_payouts(
        &self,
        profile: Profile,
        wallet_name: String,
    ) -> Result<Vec<Payout>, BriaError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        self.payouts.list_for_wallet(wallet.id).await
    }

    #[instrument(name = "app.spawn_sync_all_wallets", skip_all, err)]
    async fn spawn_sync_all_wallets(
        pool: sqlx::PgPool,
        delay: std::time::Duration,
    ) -> Result<(), BriaError> {
        tokio::spawn(async move {
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
        tokio::spawn(async move {
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
