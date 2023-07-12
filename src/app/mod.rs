mod config;
pub mod error;

use sqlxmq::OwnedHandle;
use tracing::instrument;

use std::collections::HashMap;

pub use config::*;
use error::*;

use crate::{
    account::balance::AccountBalanceSummary,
    address::*,
    batch::*,
    descriptor::*,
    fees::{self, *},
    job,
    ledger::*,
    outbox::*,
    payout::*,
    payout_queue::*,
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
    payout_queues: PayoutQueues,
    payouts: Payouts,
    batches: Batches,
    signing_sessions: SigningSessions,
    ledger: Ledger,
    utxos: Utxos,
    addresses: Addresses,
    mempool_space_client: MempoolSpaceClient,
    pool: sqlx::PgPool,
    config: AppConfig,
}

impl App {
    pub async fn run(pool: sqlx::PgPool, config: AppConfig) -> Result<Self, ApplicationError> {
        let wallets = Wallets::new(&pool);
        let xpubs = XPubs::new(&pool);
        let payout_queues = PayoutQueues::new(&pool);
        let batches = Batches::new(&pool);
        let payouts = Payouts::new(&pool);
        let ledger = Ledger::init(&pool).await?;
        let utxos = Utxos::new(&pool);
        let signing_sessions = SigningSessions::new(&pool);
        let addresses = Addresses::new(&pool);
        let outbox = Outbox::init(&pool, Augmenter::new(&addresses, &payouts)).await?;
        let mempool_space_client = MempoolSpaceClient::new(config.fees.mempool_space.clone());
        let runner = job::start_job_runner(
            &pool,
            outbox.clone(),
            wallets.clone(),
            xpubs.clone(),
            payout_queues.clone(),
            batches.clone(),
            signing_sessions.clone(),
            payouts.clone(),
            ledger.clone(),
            utxos.clone(),
            addresses.clone(),
            config.jobs.clone(),
            config.blockchain.clone(),
            config.signer_encryption.clone(),
            mempool_space_client.clone(),
        )
        .await?;
        Self::spawn_sync_all_wallets(pool.clone(), config.jobs.sync_all_wallets_delay).await?;
        Self::spawn_process_all_payout_queues(
            pool.clone(),
            config.jobs.process_all_payout_queues_delay,
        )
        .await?;
        Self::spawn_respawn_all_outbox_handlers(
            pool.clone(),
            config.jobs.respawn_all_outbox_handlers_delay,
        )
        .await?;
        Ok(Self {
            outbox,
            profiles: Profiles::new(&pool),
            xpubs,
            descriptors: Descriptors::new(&pool),
            wallets,
            payout_queues,
            payouts,
            batches,
            signing_sessions,
            pool,
            ledger,
            utxos,
            addresses,
            mempool_space_client,
            config,
            _runner: runner,
        })
    }

    #[instrument(name = "app.authenticate", skip_all, err)]
    pub async fn authenticate(&self, key: &str) -> Result<Profile, ApplicationError> {
        let profile = self.profiles.find_by_key(key).await?;
        Ok(profile)
    }

    #[instrument(name = "app.create_profile", skip(self), err)]
    pub async fn create_profile(
        &self,
        profile: Profile,
        name: String,
    ) -> Result<Profile, ApplicationError> {
        let mut tx = self.pool.begin().await?;
        let new_profile = self
            .profiles
            .create_in_tx(&mut tx, profile.account_id, name)
            .await?;
        tx.commit().await?;
        Ok(new_profile)
    }

    #[instrument(name = "app.list_profiles", skip(self), err)]
    pub async fn list_profiles(&self, profile: Profile) -> Result<Vec<Profile>, ApplicationError> {
        let profiles = self.profiles.list_for_account(profile.account_id).await?;
        Ok(profiles)
    }

    #[instrument(name = "app.create_profile_api_key", skip(self), err)]
    pub async fn create_profile_api_key(
        &self,
        profile: Profile,
        profile_name: String,
    ) -> Result<ProfileApiKey, ApplicationError> {
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
    ) -> Result<XPubId, ApplicationError> {
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
    ) -> Result<(), ApplicationError> {
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
        xpub.set_signer_config(config, &self.config.signer_encryption.key)?;
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

    #[instrument(name = "app.submit_signed_psbt", skip(self), err)]
    pub async fn submit_signed_psbt(
        &self,
        profile: Profile,
        batch_id: BatchId,
        xpub_ref: String,
        signed_psbt: bitcoin::psbt::PartiallySignedTransaction,
    ) -> Result<(), ApplicationError> {
        let xpub = self
            .xpubs
            .find_from_ref(
                profile.account_id,
                xpub_ref
                    .parse::<XPubRef>()
                    .expect("ref should always parse"),
            )
            .await?;
        let xpub_id = xpub.id();
        let xpub = xpub.value;
        let unsigned_psbt = self
            .batches
            .find_by_id(profile.account_id, batch_id)
            .await?
            .unsigned_psbt;
        psbt_validator::validate_psbt(&signed_psbt, xpub, &unsigned_psbt)?;
        let mut sessions = self
            .signing_sessions
            .list_for_batch(profile.account_id, batch_id)
            .await?
            .ok_or(ApplicationError::SigningSessionNotFoundForBatchId(batch_id))?
            .xpub_sessions;
        let session = sessions
            .get_mut(&xpub_id)
            .ok_or_else(|| ApplicationError::SigningSessionNotFoundForXPubId(xpub_id))?;

        let mut tx = self.pool.begin().await?;
        session.submit_externally_signed_psbt(signed_psbt);
        self.signing_sessions
            .update_sessions(&mut tx, &sessions)
            .await?;
        job::spawn_all_batch_signings(tx, std::iter::once((profile.account_id, batch_id))).await?;
        Ok(())
    }

    #[instrument(name = "app.create_wpkh_wallet", skip(self), err)]
    pub async fn create_wpkh_wallet(
        &self,
        profile: Profile,
        wallet_name: String,
        xpub: String,
        derivation: Option<String>,
    ) -> Result<(WalletId, Vec<XPubId>), ApplicationError> {
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
    ) -> Result<(WalletId, Vec<XPubId>), ApplicationError> {
        let keychain = KeychainConfig::try_from((external.as_ref(), internal.as_ref()))?;
        self.create_wallet(profile, wallet_name, keychain).await
    }

    #[instrument(name = "app.create_sorted_multisig_wallet", skip(self), err)]
    pub async fn create_sorted_multisig_wallet(
        &self,
        profile: Profile,
        wallet_name: String,
        xpubs: Vec<String>,
        threshold: u32,
    ) -> Result<(WalletId, Vec<XPubId>), ApplicationError> {
        let xpub_values: Vec<XPub> = futures::future::try_join_all(
            xpubs
                .iter()
                .map(|xpub| {
                    xpub.parse::<XPubRef>()
                        .expect("xpub_ref should always parse")
                })
                .map(|xpub_ref| self.xpubs.find_from_ref(profile.account_id, xpub_ref)),
        )
        .await?
        .into_iter()
        .map(|xpub| xpub.value)
        .collect();

        let keychain = KeychainConfig::sorted_multisig(xpub_values, threshold);
        self.create_wallet(profile, wallet_name, keychain).await
    }

    async fn create_wallet(
        &self,
        profile: Profile,
        wallet_name: String,
        keychain: KeychainConfig,
    ) -> Result<(WalletId, Vec<XPubId>), ApplicationError> {
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
                        .key_name(format!("{wallet_name}-{}", xpub.id()))
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
            .create_ledger_accounts_for_wallet(&mut tx, wallet_id)
            .await?;
        let new_wallet = NewWallet::builder()
            .id(wallet_id)
            .network(self.config.blockchain.network)
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
    ) -> Result<WalletBalanceSummary, ApplicationError> {
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
    ) -> Result<AccountBalanceSummary, ApplicationError> {
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
    ) -> Result<String, ApplicationError> {
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
    ) -> Result<(), ApplicationError> {
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
    ) -> Result<(WalletId, Vec<WalletAddress>), ApplicationError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        let addresses = self
            .addresses
            .list_external_by_wallet_id(profile.account_id, wallet.id)
            .await?;

        Ok((wallet.id, addresses))
    }

    #[instrument(name = "app.get_address_by_external_id", skip(self), err)]
    pub async fn find_address_by_external_id(
        &self,
        profile: Profile,
        external_id: String,
    ) -> Result<WalletAddress, ApplicationError> {
        let address = self
            .addresses
            .find_by_external_id(profile.account_id, external_id)
            .await?;
        Ok(address)
    }

    #[instrument(name = "app.get_address_by_external_id", skip(self), err)]
    pub async fn find_address(
        &self,
        profile: Profile,
        address: String,
    ) -> Result<WalletAddress, ApplicationError> {
        let address = self
            .addresses
            .find_by_address(profile.account_id, address)
            .await?;
        Ok(address)
    }

    #[instrument(name = "app.list_xpubs", skip(self), err)]
    pub async fn list_xpubs(&self, profile: Profile) -> Result<Vec<AccountXPub>, ApplicationError> {
        let xpubs = self.xpubs.list_xpubs(profile.account_id).await?;
        Ok(xpubs)
    }

    #[instrument(name = "app.list_utxos", skip(self), err)]
    pub async fn list_utxos(
        &self,
        profile: Profile,
        wallet_name: String,
    ) -> Result<(WalletId, Vec<KeychainUtxos>), ApplicationError> {
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

    #[instrument(name = "app.create_payout_queue", skip(self), err)]
    pub async fn create_payout_queue(
        &self,
        profile: Profile,
        payout_queue_name: String,
        description: Option<String>,
        config: Option<PayoutQueueConfig>,
    ) -> Result<PayoutQueueId, ApplicationError> {
        let mut builder = NewPayoutQueue::builder();
        builder
            .account_id(profile.account_id)
            .name(payout_queue_name)
            .description(description);
        if let Some(config) = config {
            builder.config(config);
        }
        let payout_queue = builder.build().expect("Couldn't build NewPayoutQueue");
        let payout_queue_id = self.payout_queues.create(payout_queue).await?;
        Ok(payout_queue_id)
    }

    #[instrument(name = "app.trigger_payout_queue", skip(self), err)]
    pub async fn trigger_payout_queue(
        &self,
        profile: Profile,
        name: String,
    ) -> Result<(), ApplicationError> {
        let payout_queue = self
            .payout_queues
            .find_by_name(profile.account_id, name)
            .await?;
        job::spawn_process_payout_queue(&self.pool, (payout_queue.account_id, payout_queue.id))
            .await?;
        Ok(())
    }

    #[instrument(name = "app.estimate_payout_fee", skip(self), ret, err)]
    pub async fn estimate_payout_fee(
        &self,
        profile: Profile,
        wallet_name: String,
        queue_name: String,
        destination: PayoutDestination,
        sats: Satoshis,
    ) -> Result<Satoshis, ApplicationError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        let payout_queue = self
            .payout_queues
            .find_by_name(profile.account_id, queue_name)
            .await?;
        let mut unbatched_payouts = self
            .payouts
            .list_unbatched(profile.account_id, payout_queue.id)
            .await?;
        let payout_id = uuid::Uuid::new_v4();
        let destination = destination
            .onchain_address()
            .expect("Destination is not onchain");
        unbatched_payouts
            .include_simulated_payout(wallet.id, (payout_id, destination.clone(), sats));

        let queue_id = payout_queue.id;
        let tx_priority = payout_queue.config.tx_priority;
        let fee_rate = self.mempool_space_client.fee_rate(tx_priority).await?;

        let psbt = {
            let mut tx = self.pool.begin().await?;
            job::process_payout_queue::construct_psbt(
                &self.pool,
                &mut tx,
                &unbatched_payouts,
                &self.utxos,
                &self.wallets,
                payout_queue,
                fee_rate,
            )
            .await?
        };

        if let Some(fee) = psbt.proportional_fee(&wallet.id, sats) {
            return Ok(fee);
        }

        // No utxos were available to simulate the batch
        let avg_utxo_size = self.utxos.average_utxo_value(wallet.id, queue_id).await?;
        let (n_payouts, payout_size) = self
            .payouts
            .average_payout_per_batch(wallet.id, queue_id)
            .await?;
        Ok(fees::estimate_proportional_fee(
            avg_utxo_size,
            wallet
                .current_keychain_wallet(&self.pool)
                .max_satisfaction_weight(),
            fee_rate,
            n_payouts,
            payout_size,
            destination,
            sats,
        ))
    }

    #[instrument(name = "app.submit_payout", skip(self), err)]
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_payout(
        &self,
        profile: Profile,
        wallet_name: String,
        queue_name: String,
        destination: PayoutDestination,
        sats: Satoshis,
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<PayoutId, ApplicationError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        let payout_queue = self
            .payout_queues
            .find_by_name(profile.account_id, queue_name)
            .await?;

        if self.config.security.is_blocked(&destination) {
            return Err(ApplicationError::DestinationBlocked(destination));
        }

        let mut builder = NewPayout::builder();
        builder
            .account_id(profile.account_id)
            .profile_id(profile.id)
            .wallet_id(wallet.id)
            .payout_queue_id(payout_queue.id)
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
            .payout_submitted(
                tx,
                LedgerTransactionId::from(uuid::Uuid::from(id)),
                PayoutSubmittedParams {
                    journal_id: wallet.journal_id,
                    effective_outgoing_account_id: wallet.ledger_account_ids.effective_outgoing_id,
                    external_id: external_id.unwrap_or_else(|| id.to_string()),
                    meta: PayoutSubmittedMeta {
                        account_id: profile.account_id,
                        payout_id: id,
                        payout_queue_id: payout_queue.id,
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

    pub async fn cancel_payout(
        &self,
        profile: Profile,
        id: PayoutId,
    ) -> Result<(), ApplicationError> {
        let mut tx = self.pool.begin().await?;
        let mut payout = self
            .payouts
            .find_by_id_for_cancellation(&mut tx, profile.account_id, id)
            .await?;
        if payout.batch_id.is_some() {
            return Err(ApplicationError::PayoutAlreadyCommitted);
        }
        if payout.is_cancelled() {
            return Ok(());
        }
        payout.cancel_payout();
        self.ledger
            .payout_cancelled(
                tx,
                LedgerTransactionId::new(),
                LedgerTransactionId::from(uuid::Uuid::from(payout.id)),
            )
            .await?;
        self.payouts.update(payout).await?;
        Ok(())
    }

    #[instrument(name = "app.list_wallets", skip_all, err)]
    pub async fn list_wallets(&self, profile: Profile) -> Result<Vec<Wallet>, ApplicationError> {
        Ok(self.wallets.list_by_account_id(profile.account_id).await?)
    }

    #[instrument(name = "app.find_payout_by_external_id", skip_all, err)]
    pub async fn find_payout_by_external_id(
        &self,
        profile: Profile,
        external_id: String,
    ) -> Result<Payout, ApplicationError> {
        Ok(self
            .payouts
            .find_by_external_id(profile.account_id, external_id)
            .await?)
    }

    #[instrument(name = "app.find_payout", skip_all, err)]
    pub async fn find_payout(
        &self,
        profile: Profile,
        id: PayoutId,
    ) -> Result<Payout, ApplicationError> {
        Ok(self.payouts.find_by_id(profile.account_id, id).await?)
    }

    #[instrument(name = "app.list_payouts", skip_all, err)]
    pub async fn list_payouts(
        &self,
        profile: Profile,
        wallet_name: String,
    ) -> Result<Vec<Payout>, ApplicationError> {
        let wallet = self
            .wallets
            .find_by_name(profile.account_id, wallet_name)
            .await?;
        Ok(self
            .payouts
            .list_for_wallet(profile.account_id, wallet.id)
            .await?)
    }

    #[instrument(name = "app.list_payout_queues", skip_all, err)]
    pub async fn list_payout_queues(
        &self,
        profile: Profile,
    ) -> Result<Vec<PayoutQueue>, ApplicationError> {
        let payout_queues = self
            .payout_queues
            .list_by_account_id(profile.account_id)
            .await?;
        Ok(payout_queues)
    }

    #[instrument(name = "app.update_payout_queue", skip(self), err)]
    pub async fn update_payout_queue(
        &self,
        profile: Profile,
        id: PayoutQueueId,
        new_description: Option<String>,
        new_config: Option<PayoutQueueConfig>,
    ) -> Result<(), ApplicationError> {
        let mut payout_queue = self
            .payout_queues
            .find_by_id(profile.account_id, id)
            .await?;
        if let Some(desc) = new_description {
            payout_queue.update_description(desc)
        }
        if let Some(config) = new_config {
            payout_queue.update_config(config)
        }
        self.payout_queues.update(payout_queue).await?;
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    #[instrument(name = "app.get_batch", skip_all, err)]
    pub async fn get_batch(
        &self,
        profile: Profile,
        batch_id: BatchId,
    ) -> Result<
        (
            Batch,
            HashMap<WalletId, Vec<Payout>>,
            Option<BatchSigningSession>,
        ),
        ApplicationError,
    > {
        let batch = self
            .batches
            .find_by_id(profile.account_id, batch_id)
            .await?;
        let payouts = self
            .payouts
            .list_for_batch(profile.account_id, batch_id)
            .await?;
        let signing_sessions = self
            .signing_sessions
            .list_for_batch(profile.account_id, batch_id)
            .await?;
        Ok((batch, payouts, signing_sessions))
    }

    #[instrument(name = "app.subscribe_all", skip(self), err)]
    pub async fn subscribe_all(
        &self,
        profile: Profile,
        start_after: Option<u64>,
        augment: bool,
    ) -> Result<OutboxListener, ApplicationError> {
        let res = self
            .outbox
            .register_listener(
                profile.account_id,
                start_after.map(EventSequence::from),
                augment,
            )
            .await?;
        Ok(res)
    }

    #[instrument(name = "app.spawn_sync_all_wallets", skip_all, err)]
    async fn spawn_sync_all_wallets(
        pool: sqlx::PgPool,
        delay: std::time::Duration,
    ) -> Result<(), ApplicationError> {
        tokio::spawn(async move {
            loop {
                let _ = job::spawn_sync_all_wallets(&pool, std::time::Duration::from_secs(1)).await;
                tokio::time::sleep(delay).await;
            }
        });
        Ok(())
    }

    #[instrument(name = "app.spawn_process_all_payout_queues", skip_all, err)]
    async fn spawn_process_all_payout_queues(
        pool: sqlx::PgPool,
        delay: std::time::Duration,
    ) -> Result<(), ApplicationError> {
        tokio::spawn(async move {
            loop {
                let _ =
                    job::spawn_process_all_payout_queues(&pool, std::time::Duration::from_secs(1))
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
    ) -> Result<(), ApplicationError> {
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
