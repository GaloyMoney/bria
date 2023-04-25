mod config;

use sqlxmq::OwnedHandle;
use tracing::instrument;

pub use config::*;

use crate::{
    address::*,
    batch::*,
    batch_group::*,
    error::*,
    job,
    ledger::*,
    payout::*,
    primitives::*,
    profile::*,
    signing_session::*,
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
    signing_sessions: SigningSessions,
    ledger: Ledger,
    utxos: Utxos,
    addresses: Addresses,
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
        let wallets = Wallets::new(&pool);
        let xpubs = XPubs::new(&pool);
        let batch_groups = BatchGroups::new(&pool);
        let batches = Batches::new(&pool);
        let payouts = Payouts::new(&pool);
        let ledger = Ledger::init(&pool).await?;
        let utxos = Utxos::new(&pool);
        let signing_sessions = SigningSessions::new(&pool);
        let addresses = Addresses::new(&pool);
        let runner = job::start_job_runner(
            &pool,
            wallets.clone(),
            xpubs.clone(),
            batch_groups.clone(),
            batches,
            signing_sessions.clone(),
            payouts.clone(),
            ledger.clone(),
            utxos.clone(),
            addresses.clone(),
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
            xpubs,
            wallets,
            batch_groups,
            payouts,
            signing_sessions,
            pool,
            ledger,
            utxos,
            addresses,
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
        let value = XPub::try_from((&xpub, derivation))?;
        let xpub = NewAccountXPub::builder()
            .account_id(profile.account_id)
            .original(xpub)
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
        let mut xpub = self
            .xpubs
            .find_from_ref(
                profile.account_id,
                xpub_ref
                    .parse::<XPubRef>()
                    .expect("ref should always parse"),
            )
            .await?;
        let xpub_id = xpub.id();
        xpub.set_signer_config(config);
        let mut tx = self.pool.begin().await?;
        self.xpubs.persist_updated(&mut tx, xpub).await?;
        let batch_ids = self
            .signing_sessions
            .list_batch_ids_for(&mut tx, profile.account_id, xpub_id)
            .await?;
        job::spawn_all_batch_signings(tx, batch_ids.into_iter().map(|b| (profile.account_id, b)))
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
                    .find_from_ref(
                        profile.account_id,
                        xpub_ref
                            .parse::<XPubRef>()
                            .expect("xpub_ref should always parse"),
                    )
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
            .network(self.blockchain_cfg.network)
            .account_id(profile.account_id)
            .journal_id(uuid::Uuid::from(profile.account_id))
            .name(wallet_name.clone())
            .keychain(WpkhKeyChainConfig::new(
                xpubs.into_iter().next().expect("xpubs is empty").value,
            ))
            .ledger_account_ids(wallet_ledger_accounts)
            .build()
            .expect("Couldn't build NewWallet");
        let wallet_id = self.wallets.create_in_tx(&mut tx, new_wallet).await?;

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
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<String, BriaError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        let keychain_wallet = wallet.current_keychain_wallet(&self.pool);
        let addr = keychain_wallet.new_external_address().await?;

        let mut builder = NewAddress::builder();
        builder
            .address(addr.address.clone())
            .account_id(profile.account_id)
            .wallet_id(wallet.id)
            .profile_id(profile.id)
            .keychain_id(keychain_wallet.keychain_id)
            .kind(bitcoin::KeychainKind::External)
            .address_idx(addr.index)
            .metadata(metadata);
        if let Some(external_id) = external_id {
            builder.external_id(external_id);
        }
        let new_address = builder.build().expect("Couldn't build NewAddress");
        self.addresses.persist_address(new_address).await?;

        Ok(addr.to_string())
    }

    #[instrument(name = "app.list_addresses", skip(self), err)]
    pub async fn list_external_addresses(
        &self,
        profile: Profile,
        wallet_name: String,
    ) -> Result<(WalletId, Vec<WalletAddress>), BriaError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        let addresses = self
            .addresses
            .find_external_by_wallet_id(profile.account_id, wallet.id)
            .await?;

        Ok((wallet.id, addresses))
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
        let batch_group = self
            .batch_groups
            .find_by_name(profile.account_id, group_name)
            .await?;
        let mut builder = NewPayout::builder();
        builder
            .account_id(profile.account_id)
            .profile_id(profile.id)
            .wallet_id(wallet.id)
            .batch_group_id(batch_group.id)
            .destination(destination.clone())
            .satoshis(sats)
            .metadata(metadata.clone());
        if let Some(external_id) = external_id.as_ref() {
            builder.external_id(external_id);
        }
        let new_payout = builder.build().expect("Couldn't build NewPayout");
        let mut tx = self.pool.begin().await?;
        let id = self.payouts.create_in_tx(&mut tx, new_payout).await?;
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
                        batch_group_id: batch_group.id,
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
        self.payouts
            .list_for_wallet(profile.account_id, wallet.id)
            .await
    }

    #[instrument(name = "app.list_signing_sessions", skip_all, err)]
    pub async fn list_signing_sessions(
        &self,
        profile: Profile,
        batch_id: BatchId,
    ) -> Result<Vec<SigningSession>, BriaError> {
        Ok(self
            .signing_sessions
            .find_for_batch(profile.account_id, batch_id)
            .await?
            .map(|BatchSigningSession { xpub_sessions }| xpub_sessions.into_values().collect())
            .unwrap_or_default())
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
