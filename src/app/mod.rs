mod config;

use sqlxmq::OwnedHandle;
use tracing::instrument;

pub use config::*;

use crate::{
    account::balance::AccountBalanceSummary,
    address::*,
    batch::*,
    batch_group::*,
    descriptor::*,
    error::*,
    job,
    ledger::*,
    outbox::*,
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
    outbox: Outbox,
    profiles: Profiles,
    xpubs: XPubs,
    descriptors: Descriptors,
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
        blockchain_cfg: BlockchainConfig,
        app_cfg: AppConfig,
    ) -> Result<Self, BriaError> {
        let wallets = Wallets::new(&pool);
        let xpubs = XPubs::new(&pool);
        let batch_groups = BatchGroups::new(&pool);
        let batches = Batches::new(&pool);
        let payouts = Payouts::new(&pool);
        let ledger = Ledger::init(&pool).await?;
        let utxos = Utxos::new(&pool);
        let signing_sessions = SigningSessions::new(&pool);
        let addresses = Addresses::new(&pool);
        let outbox = Outbox::init(&pool, Augmenter::new(&addresses, &payouts)).await?;
        let runner = job::start_job_runner(
            &pool,
            outbox.clone(),
            wallets.clone(),
            xpubs.clone(),
            batch_groups.clone(),
            batches,
            signing_sessions.clone(),
            payouts.clone(),
            ledger.clone(),
            utxos.clone(),
            addresses.clone(),
            app_cfg.sync_all_wallets_delay,
            app_cfg.process_all_batch_groups_delay,
            app_cfg.respawn_all_outbox_handlers_delay,
            blockchain_cfg.clone(),
        )
        .await?;
        Self::spawn_sync_all_wallets(pool.clone(), app_cfg.sync_all_wallets_delay).await?;
        Self::spawn_process_all_batch_groups(pool.clone(), app_cfg.process_all_batch_groups_delay)
            .await?;
        Self::spawn_respawn_all_outbox_handlers(
            pool.clone(),
            app_cfg.respawn_all_outbox_handlers_delay,
        )
        .await?;
        Ok(Self {
            outbox,
            profiles: Profiles::new(&pool),
            xpubs,
            descriptors: Descriptors::new(&pool),
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
            .create_key_for_profile_in_tx(&mut tx, found_profile, false)
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

    #[instrument(name = "app.create_wpkh_wallet", skip(self), err)]
    pub async fn create_wpkh_wallet(
        &self,
        profile: Profile,
        wallet_name: String,
        xpub: String,
        derivation: Option<String>,
    ) -> Result<(WalletId, Vec<XPubId>), BriaError> {
        let keychain = if let Ok(xpub) = XPub::try_from((&xpub, derivation)) {
            KeychainConfig::wpkh(xpub)
        } else {
            KeychainConfig::wpkh(
                self.xpubs
                    .find_from_ref(
                        profile.account_id,
                        xpub.parse::<XPubRef>()
                            .expect("xpub_ref should always parse"),
                    )
                    .await?
                    .value,
            )
        };
        self.create_wallet(profile, wallet_name, keychain).await
    }

    #[instrument(name = "app.create_descriptors_wallet", skip(self), err)]
    pub async fn create_descriptors_wallet(
        &self,
        profile: Profile,
        wallet_name: String,
        external: String,
        internal: String,
    ) -> Result<(WalletId, Vec<XPubId>), BriaError> {
        let keychain = KeychainConfig::try_from((external.as_ref(), internal.as_ref()))?;
        self.create_wallet(profile, wallet_name, keychain).await
    }

    async fn create_wallet(
        &self,
        profile: Profile,
        wallet_name: String,
        keychain: KeychainConfig,
    ) -> Result<(WalletId, Vec<XPubId>), BriaError> {
        let mut tx = self.pool.begin().await?;
        let xpubs = keychain.xpubs();
        let mut xpub_ids = Vec::new();
        for xpub in xpubs {
            match self
                .xpubs
                .find_from_ref(profile.account_id, xpub.id())
                .await
            {
                Ok(xpub) => {
                    xpub_ids.push(xpub.id());
                }
                Err(_) => {
                    let original = xpub.inner().to_string();
                    let xpub = NewAccountXPub::builder()
                        .account_id(profile.account_id)
                        .key_name(original.clone())
                        .original(original)
                        .value(xpub)
                        .build()
                        .expect("Couldn't build xpub");
                    xpub_ids.push(self.xpubs.persist_in_tx(&mut tx, xpub).await?);
                }
            }
        }
        let wallet_id = WalletId::new();
        let wallet_ledger_accounts = self
            .ledger
            .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &wallet_name)
            .await?;
        let new_wallet = NewWallet::builder()
            .id(wallet_id)
            .network(self.blockchain_cfg.network)
            .account_id(profile.account_id)
            .journal_id(profile.account_id)
            .name(wallet_name)
            .keychain(keychain.clone())
            .ledger_account_ids(wallet_ledger_accounts)
            .build()
            .expect("Couldn't build NewWallet");
        let wallet_id = self.wallets.create_in_tx(&mut tx, new_wallet).await?;
        let descriptors = vec![
            NewDescriptor::builder()
                .account_id(profile.account_id)
                .wallet_id(wallet_id)
                .descriptor(keychain.external_descriptor())
                .keychain_kind(bitcoin::KeychainKind::External)
                .build()
                .expect("Could not build descriptor"),
            NewDescriptor::builder()
                .account_id(profile.account_id)
                .wallet_id(wallet_id)
                .descriptor(keychain.internal_descriptor())
                .keychain_kind(bitcoin::KeychainKind::Internal)
                .build()
                .expect("Could not build descriptor"),
        ];
        self.descriptors
            .persist_all_in_tx(&mut tx, descriptors)
            .await?;
        tx.commit().await?;
        Ok((wallet_id, xpub_ids))
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

    #[instrument(name = "app.get_account_balance_summary", skip(self), err)]
    pub async fn get_account_balance_summary(
        &self,
        profile: Profile,
    ) -> Result<AccountBalanceSummary, BriaError> {
        let account_ledger_account_balances = self
            .ledger
            .get_account_ledger_account_balances(profile.account_id.into())
            .await?;
        let summary = AccountBalanceSummary::from(account_ledger_account_balances);
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
        self.addresses.persist_new_address(new_address).await?;

        Ok(addr.to_string())
    }

    #[instrument(name = "app.update_address", skip(self), err)]
    pub async fn update_address(
        &self,
        profile: Profile,
        address: String,
        new_external_id: Option<String>,
        new_metadata: Option<serde_json::Value>,
    ) -> Result<(), BriaError> {
        let mut address = self
            .addresses
            .find_by_address(profile.account_id, address)
            .await?;
        if let Some(id) = new_external_id {
            address.update_external_id(id);
        }
        if let Some(metadata) = new_metadata {
            address.update_metadata(metadata);
        }
        self.addresses.update(address).await?;
        Ok(())
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

    #[instrument(name = "app.list_xpubs", skip(self), err)]
    pub async fn list_xpubs(&self, profile: Profile) -> Result<Vec<AccountXPub>, BriaError> {
        let xpubs = self.xpubs.list_xpubs(profile.account_id).await?;
        Ok(xpubs)
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
            .payout_queued(
                tx,
                LedgerTransactionId::from(uuid::Uuid::from(id)),
                PayoutQueuedParams {
                    journal_id: wallet.journal_id,
                    logical_outgoing_account_id: wallet.ledger_account_ids.logical_outgoing_id,
                    external_id: external_id.unwrap_or_else(|| id.to_string()),
                    meta: PayoutQueuedMeta {
                        account_id: profile.account_id,
                        payout_id: id,
                        batch_group_id: batch_group.id,
                        wallet_id: wallet.id,
                        profile_id: profile.id,
                        satoshis: sats,
                        destination,
                    },
                },
            )
            .await?;
        Ok(id)
    }

    #[instrument(name = "app.list_wallets", skip_all, err)]
    pub async fn list_wallets(&self, profile: Profile) -> Result<Vec<Wallet>, BriaError> {
        self.wallets.list_by_account_id(profile.account_id).await
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

    #[instrument(name = "app.list_batch_groups", skip_all, err)]
    pub async fn list_batch_groups(&self, profile: Profile) -> Result<Vec<BatchGroup>, BriaError> {
        let batch_groups = self
            .batch_groups
            .list_by_account_id(profile.account_id)
            .await?;
        Ok(batch_groups)
    }

    #[instrument(name = "app.update_batch_group", skip(self), err)]
    pub async fn update_batch_group(
        &self,
        profile: Profile,
        id: BatchGroupId,
        new_description: Option<String>,
        new_config: Option<BatchGroupConfig>,
    ) -> Result<(), BriaError> {
        let mut batch_group = self.batch_groups.find_by_id(profile.account_id, id).await?;
        if let Some(desc) = new_description {
            batch_group.update_description(desc)
        }
        if let Some(config) = new_config {
            batch_group.update_config(config)
        }
        self.batch_groups.update(batch_group).await?;
        Ok(())
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

    #[instrument(name = "app.subscribe_all", skip(self), err)]
    pub async fn subscribe_all(
        &self,
        profile: Profile,
        start_after: Option<u64>,
        augment: bool,
    ) -> Result<OutboxListener, BriaError> {
        self.outbox
            .register_listener(
                profile.account_id,
                start_after.map(EventSequence::from),
                augment,
            )
            .await
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

    #[instrument(name = "app.spawn_respawn_all_outbox_handlers", skip_all, err)]
    async fn spawn_respawn_all_outbox_handlers(
        pool: sqlx::PgPool,
        delay: std::time::Duration,
    ) -> Result<(), BriaError> {
        tokio::spawn(async move {
            loop {
                let _ = job::spawn_respawn_all_outbox_handlers(
                    &pool,
                    std::time::Duration::from_secs(1),
                )
                .await;
                tokio::time::sleep(delay).await;
            }
        });
        Ok(())
    }
}
