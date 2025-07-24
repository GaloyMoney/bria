mod config;
pub mod error;

use sqlxmq::JobRunnerHandle;
use tracing::instrument;

use std::collections::HashMap;

pub use config::*;
use error::*;

use crate::{
    account::balance::AccountBalanceSummary,
    address::*,
    batch::*,
    batch_inclusion::*,
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
    _runner: JobRunnerHandle,
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
    fees_client: FeesClient,
    batch_inclusion: BatchInclusion,
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
        let batch_inclusion = BatchInclusion::new(pool.clone(), payout_queues.clone());
        let outbox = Outbox::init(
            &pool,
            Augmenter::new(&addresses, &payouts, &batch_inclusion),
        )
        .await?;
        let fees_client = FeesClient::new(config.fees.clone());
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
            fees_client.clone(),
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
        let app = Self {
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
            fees_client,
            batch_inclusion,
            config,
            _runner: runner,
        };
        if let Some(deprecrated_encryption_key) = app.config.deprecated_encryption_key.as_ref() {
            app.rotate_encryption_key(deprecrated_encryption_key)
                .await?;
        }
        Ok(app)
    }

    pub fn network(&self) -> bitcoin::Network {
        self.config.blockchain.network
    }

    #[instrument(name = "app.authenticate", skip_all, err)]
    pub async fn authenticate(&self, key: &str) -> Result<Profile, ApplicationError> {
        let profile = self.profiles.find_by_key(key).await?;
        Ok(profile)
    }

    #[instrument(name = "app.create_profile", skip(self), err)]
    pub async fn create_profile(
        &self,
        profile: &Profile,
        name: String,
        spending_policy: Option<SpendingPolicy>,
    ) -> Result<Profile, ApplicationError> {
        let new_profile = NewProfile::builder()
            .account_id(profile.account_id)
            .name(name)
            .spending_policy(spending_policy)
            .build()
            .expect("Couldn't build NewProfile");
        let new_profile = self.profiles.create(new_profile).await?;
        Ok(new_profile)
    }

    #[instrument(name = "app.update_profile", skip(self), err)]
    pub async fn update_profile(
        &self,
        profile: &Profile,
        profile_id: ProfileId,
        spending_policy: Option<SpendingPolicy>,
    ) -> Result<(), ApplicationError> {
        let mut target_profile = self
            .profiles
            .find_by_account_id_and_id(profile.account_id, profile_id)
            .await?;
        target_profile.update_spending_policy(spending_policy);
        self.profiles.update(&mut target_profile).await?;
        Ok(())
    }

    #[instrument(name = "app.list_profiles", skip(self), err)]
    pub async fn list_profiles(&self, profile: &Profile) -> Result<Vec<Profile>, ApplicationError> {
        let profiles = self.profiles.list_for_account(profile.account_id).await?;
        Ok(profiles)
    }

    #[instrument(name = "app.create_profile_api_key", skip(self), err)]
    pub async fn create_profile_api_key(
        &self,
        profile: &Profile,
        profile_name: String,
    ) -> Result<ProfileApiKey, ApplicationError> {
        let found_profile = self
            .profiles
            .find_by_account_id_and_name(profile.account_id, profile_name)
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
        profile: &Profile,
        key_name: String,
        xpub: String,
        derivation: Option<String>,
    ) -> Result<XPubFingerprint, ApplicationError> {
        let value = XPub::try_from((&xpub, derivation))?;
        let xpub = NewAccountXPub::builder()
            .account_id(profile.account_id)
            .original(xpub)
            .key_name(key_name)
            .value(value)
            .build()
            .expect("Couldn't build xpub");
        let fingerprint = self.xpubs.create(xpub).await?.fingerprint();
        Ok(fingerprint)
    }

    #[instrument(name = "app.set_signer_config", skip(self), err)]
    pub async fn set_signer_config(
        &self,
        profile: &Profile,
        xpub_ref: String,
        config: SignerConfig,
    ) -> Result<(), ApplicationError> {
        let mut db = self.xpubs.begin_op().await?;
        let mut xpub = self
            .xpubs
            .find_from_ref(
                profile.account_id,
                xpub_ref
                    .parse::<XPubRef>()
                    .expect("ref should always parse"),
            )
            .await?;
        let xpub_fingerprint = xpub.fingerprint();
        xpub.set_signer_config(config, &self.config.signer_encryption.key)?;
        self.xpubs.update_signer_config(&mut db, xpub).await?;
        let batch_ids = self
            .signing_sessions
            .list_batch_ids_for(db.tx(), profile.account_id, xpub_fingerprint)
            .await?;
        job::spawn_all_batch_signings(
            db.into_tx(),
            batch_ids.into_iter().map(|b| (profile.account_id, b)),
        )
        .await?;
        Ok(())
    }

    #[instrument(name = "app.rotate_encryption_key", skip_all, err)]
    pub async fn rotate_encryption_key(
        &self,
        deprecated_encryption_key: &DeprecatedEncryptionKey,
    ) -> Result<(), ApplicationError> {
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305,
        };
        let cipher = ChaCha20Poly1305::new(&self.config.signer_encryption.key);
        let nonce_bytes = hex::decode(&deprecated_encryption_key.nonce)?;
        let nonce = chacha20poly1305::Nonce::from_slice(nonce_bytes.as_slice());
        let deprecated_encrypted_key_bytes = hex::decode(&deprecated_encryption_key.key)?;
        let deprecated_key_bytes =
            cipher.decrypt(nonce, deprecated_encrypted_key_bytes.as_slice())?;
        let deprecated_key = chacha20poly1305::Key::clone_from_slice(deprecated_key_bytes.as_ref());
        let xpubs = self.xpubs.list_all_xpubs().await?;
        let mut db = self.xpubs.begin_op().await?;
        for mut xpub in xpubs {
            if let Some(signing_cfg) = xpub.signing_cfg(deprecated_key) {
                xpub.set_signer_config(signing_cfg, &self.config.signer_encryption.key)?;
                self.xpubs.update_signer_config(&mut db, xpub).await?;
            }
        }
        db.commit().await?;
        Ok(())
    }

    #[instrument(name = "app.submit_signed_psbt", skip(self), err)]
    pub async fn submit_signed_psbt(
        &self,
        profile: &Profile,
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
        let xpub_fingerprint = xpub.fingerprint();
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
        let session = sessions.get_mut(&xpub_fingerprint).ok_or_else(|| {
            ApplicationError::SigningSessionNotFoundForXPubFingerprint(xpub_fingerprint)
        })?;

        let mut db = self.signing_sessions.begin_op().await?;
        session.submit_externally_signed_psbt(signed_psbt);
        self.signing_sessions
            .update_sessions(&mut db, &sessions)
            .await?;
        job::spawn_all_batch_signings(
            db.into_tx(),
            std::iter::once((profile.account_id, batch_id)),
        )
        .await?;
        Ok(())
    }

    #[instrument(name = "app.create_wpkh_wallet", skip(self), err)]
    pub async fn create_wpkh_wallet(
        &self,
        profile: &Profile,
        wallet_name: String,
        xpub: String,
        derivation: Option<String>,
    ) -> Result<(WalletId, Vec<XPubFingerprint>), ApplicationError> {
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
        profile: &Profile,
        wallet_name: String,
        external: String,
        internal: String,
    ) -> Result<(WalletId, Vec<XPubFingerprint>), ApplicationError> {
        let keychain = KeychainConfig::try_from((external.as_ref(), internal.as_ref()))?;
        self.create_wallet(profile, wallet_name, keychain).await
    }

    #[instrument(name = "app.create_sorted_multisig_wallet", skip(self), err)]
    pub async fn create_sorted_multisig_wallet(
        &self,
        profile: &Profile,
        wallet_name: String,
        xpubs: Vec<String>,
        threshold: u32,
    ) -> Result<(WalletId, Vec<XPubFingerprint>), ApplicationError> {
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
        profile: &Profile,
        wallet_name: String,
        keychain: KeychainConfig,
    ) -> Result<(WalletId, Vec<XPubFingerprint>), ApplicationError> {
        let mut op = self.wallets.begin_op().await?;
        let xpubs = keychain.xpubs();
        let mut xpub_fingerprints = Vec::new();
        for xpub in xpubs {
            match self
                .xpubs
                .find_from_ref(profile.account_id, xpub.fingerprint())
                .await
            {
                Ok(xpub) => {
                    xpub_fingerprints.push(xpub.fingerprint());
                }
                Err(_) => {
                    let original = xpub.inner().to_string();
                    let xpub = NewAccountXPub::builder()
                        .account_id(profile.account_id)
                        .key_name(format!("{wallet_name}-{}", xpub.fingerprint()))
                        .original(original)
                        .value(xpub)
                        .build()
                        .expect("Couldn't build xpub");
                    xpub_fingerprints
                        .push(self.xpubs.create_in_op(&mut op, xpub).await?.fingerprint());
                }
            }
        }
        let wallet_id = WalletId::new();
        let wallet_ledger_accounts = self
            .ledger
            .create_ledger_accounts_for_wallet(op.tx(), wallet_id)
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
        let wallet = self.wallets.create_in_op(&mut op, new_wallet).await?;
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
            .persist_all_in_tx(op.tx(), descriptors)
            .await?;
        op.commit().await?;
        Ok((wallet.id, xpub_fingerprints))
    }

    #[instrument(name = "app.get_wallet_balance_summary", skip(self), err)]
    pub async fn get_wallet_balance_summary(
        &self,
        profile: &Profile,
        wallet_name: String,
    ) -> Result<WalletBalanceSummary, ApplicationError> {
        let wallet = self
            .wallets
            .find_by_account_id_and_name(profile.account_id, wallet_name)
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
        profile: &Profile,
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
        profile: &Profile,
        wallet_name: String,
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<(WalletId, Address), ApplicationError> {
        let wallet = self
            .wallets
            .find_by_account_id_and_name(profile.account_id, wallet_name)
            .await?;
        let keychain_wallet = wallet.current_keychain_wallet(&self.pool);
        let addr = keychain_wallet.new_external_address().await?;
        let address = Address::from(addr.address);
        let mut builder = NewWalletAddress::builder();
        builder
            .address(address.clone())
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
        self.addresses.create(new_address).await?;

        Ok((wallet.id, address))
    }

    #[instrument(name = "app.update_address", skip(self), err)]
    pub async fn update_address(
        &self,
        profile: &Profile,
        address: String,
        new_external_id: Option<String>,
        new_metadata: Option<serde_json::Value>,
    ) -> Result<(), ApplicationError> {
        let mut address = self
            .addresses
            .find_by_account_id_and_address(profile.account_id, address)
            .await?;
        if let Some(id) = new_external_id {
            address.update_external_id(id);
        }
        if let Some(metadata) = new_metadata {
            address.update_metadata(metadata);
        }
        self.addresses.update(&mut address).await?;
        Ok(())
    }

    #[instrument(name = "app.list_addresses", skip(self), err)]
    pub async fn list_external_addresses(
        &self,
        profile: &Profile,
        wallet_name: String,
    ) -> Result<(WalletId, Vec<WalletAddress>), ApplicationError> {
        let wallet = self
            .wallets
            .find_by_account_id_and_name(profile.account_id, wallet_name)
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
        profile: &Profile,
        external_id: String,
    ) -> Result<WalletAddress, ApplicationError> {
        let address = self
            .addresses
            .find_by_account_id_and_external_id(profile.account_id, external_id)
            .await?;
        Ok(address)
    }

    #[instrument(name = "app.get_address_by_external_id", skip(self), err)]
    pub async fn find_address(
        &self,
        profile: &Profile,
        address: String,
    ) -> Result<WalletAddress, ApplicationError> {
        let address = self
            .addresses
            .find_by_account_id_and_address(profile.account_id, address)
            .await?;
        Ok(address)
    }

    #[instrument(name = "app.list_xpubs", skip(self), err)]
    pub async fn list_xpubs(
        &self,
        profile: &Profile,
    ) -> Result<Vec<AccountXPub>, ApplicationError> {
        let xpubs = self.xpubs.list_xpubs(profile.account_id).await?;
        Ok(xpubs)
    }

    #[instrument(name = "app.list_utxos", skip(self), err)]
    pub async fn list_utxos(
        &self,
        profile: &Profile,
        wallet_name: String,
    ) -> Result<(WalletId, Vec<KeychainUtxos>), ApplicationError> {
        let wallet = self
            .wallets
            .find_by_account_id_and_name(profile.account_id, wallet_name)
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
        profile: &Profile,
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
        let payout_queue = self.payout_queues.create(payout_queue).await?;
        Ok(payout_queue.id)
    }

    #[instrument(name = "app.trigger_payout_queue", skip(self), err)]
    pub async fn trigger_payout_queue(
        &self,
        profile: &Profile,
        name: String,
    ) -> Result<(), ApplicationError> {
        let payout_queue = self
            .payout_queues
            .find_by_account_id_and_name(profile.account_id, name)
            .await?;
        job::spawn_process_payout_queue(&self.pool, (payout_queue.account_id, payout_queue.id))
            .await?;
        Ok(())
    }

    #[instrument(name = "app.estimate_payout_fee_to_wallet", skip(self), ret, err)]
    pub async fn estimate_payout_fee_to_wallet(
        &self,
        profile: &Profile,
        wallet_name: String,
        queue_name: String,
        destination_wallet_name: String,
        sats: Satoshis,
    ) -> Result<Satoshis, ApplicationError> {
        let destination_wallet = self
            .wallets
            .find_by_account_id_and_name(profile.account_id, destination_wallet_name)
            .await?;
        let destination = destination_wallet
            .current_keychain_wallet(&self.pool)
            .example_address()
            .await?;
        self.estimate_payout_fee_to_address(
            profile,
            wallet_name,
            queue_name,
            destination.address.to_string(),
            sats,
        )
        .await
    }

    #[instrument(name = "app.estimate_payout_fee_to_address", skip(self), ret, err)]
    pub async fn estimate_payout_fee_to_address(
        &self,
        profile: &Profile,
        wallet_name: String,
        queue_name: String,
        destination: String,
        sats: Satoshis,
    ) -> Result<Satoshis, ApplicationError> {
        let wallet = self
            .wallets
            .find_by_account_id_and_name(profile.account_id, wallet_name)
            .await?;
        let payout_queue = self
            .payout_queues
            .find_by_account_id_and_name(profile.account_id, queue_name)
            .await?;
        let mut tx = self.pool.begin().await?;
        let mut unbatched_payouts = self
            .payouts
            .list_unbatched(&mut tx, profile.account_id, payout_queue.id)
            .await?;
        let destination = Address::try_from((destination, self.config.blockchain.network))?;
        let payout_id = uuid::Uuid::new_v4();
        unbatched_payouts
            .include_simulated_payout(wallet.id, (payout_id, destination.clone(), sats));

        let queue_id = payout_queue.id;
        let tx_priority = payout_queue.config.tx_priority;
        let fee_rate = self.fees_client.fee_rate(tx_priority).await?;

        let psbt = {
            job::process_payout_queue::construct_psbt(
                &self.pool,
                &mut tx,
                &unbatched_payouts,
                &self.utxos,
                &self.wallets,
                payout_queue,
                fee_rate,
                true,
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

    #[instrument(name = "app.submit_payout_to_address", skip(self), err)]
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_payout_to_address(
        &self,
        profile: &Profile,
        wallet_name: String,
        queue_name: String,
        address: String,
        sats: Satoshis,
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<(PayoutId, Option<chrono::DateTime<chrono::Utc>>), ApplicationError> {
        let wallet = self
            .wallets
            .find_by_account_id_and_name(profile.account_id, wallet_name)
            .await?;
        let payout_queue = self
            .payout_queues
            .find_by_account_id_and_name(profile.account_id, queue_name)
            .await?;
        let addr = Address::try_from((address, self.config.blockchain.network))?;
        self.submit_payout(
            profile,
            wallet,
            payout_queue,
            PayoutId::new(),
            PayoutDestination::OnchainAddress { value: addr },
            sats,
            external_id,
            metadata,
        )
        .await
    }

    #[instrument(name = "app.submit_payout_to_wallet", skip(self), err)]
    #[allow(clippy::too_many_arguments)]
    pub async fn submit_payout_to_wallet(
        &self,
        profile: &Profile,
        wallet_name: String,
        queue_name: String,
        destination_wallet_name: String,
        sats: Satoshis,
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<(PayoutId, Option<chrono::DateTime<chrono::Utc>>), ApplicationError> {
        let wallet = self
            .wallets
            .find_by_account_id_and_name(profile.account_id, wallet_name)
            .await?;
        let payout_queue = self
            .payout_queues
            .find_by_account_id_and_name(profile.account_id, queue_name)
            .await?;
        let payout_id = PayoutId::new();
        let (wallet_id, address) = self
            .new_address(
                profile,
                destination_wallet_name.clone(),
                Some(external_id.clone().unwrap_or_else(|| payout_id.to_string())),
                metadata.clone(),
            )
            .await?;
        self.submit_payout(
            profile,
            wallet,
            payout_queue,
            payout_id,
            PayoutDestination::Wallet {
                id: wallet_id,
                address,
            },
            sats,
            external_id,
            metadata,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn submit_payout(
        &self,
        profile: &Profile,
        wallet: Wallet,
        payout_queue: PayoutQueue,
        id: PayoutId,
        destination: PayoutDestination,
        sats: Satoshis,
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<(PayoutId, Option<chrono::DateTime<chrono::Utc>>), ApplicationError> {
        if self.config.security.is_blocked(&destination) {
            return Err(ApplicationError::DestinationBlocked(destination));
        }
        if !profile.is_destination_allowed(&destination) {
            return Err(ApplicationError::DestinationNotAllowed(destination));
        }
        if !profile.is_amount_allowed(sats) {
            return Err(ApplicationError::PayoutExceedsMaximum(sats));
        }

        let mut builder = NewPayout::builder(id);
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
        let mut db = self.payouts.begin_op().await?;
        let id = self.payouts.create_in_op(&mut db, new_payout).await?.id;
        self.ledger
            .payout_submitted(
                db.into_tx(),
                id,
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

        let estimation = self
            .batch_inclusion
            .estimate_next_queue_trigger(payout_queue)
            .await?;
        Ok((id, estimation))
    }

    pub async fn cancel_payout(
        &self,
        profile: &Profile,
        id: PayoutId,
    ) -> Result<(), ApplicationError> {
        let mut db = self.payouts.begin_op().await?;
        let mut payout = self
            .payouts
            .find_by_id_for_cancellation(db.tx(), profile.account_id, id)
            .await?;
        payout.cancel_payout(profile.id)?;
        self.payouts.update_in_op(&mut db, &mut payout).await?;
        self.ledger
            .payout_cancelled(db.into_tx(), LedgerTransactionId::new(), id)
            .await?;
        Ok(())
    }

    #[instrument(name = "app.list_wallets", skip_all, err)]
    pub async fn list_wallets(&self, profile: &Profile) -> Result<Vec<Wallet>, ApplicationError> {
        Ok(self.wallets.list_for_account(profile.account_id).await?)
    }

    #[instrument(name = "app.find_payout_by_external_id", skip_all, err)]
    pub async fn find_payout_by_external_id(
        &self,
        profile: &Profile,
        external_id: String,
    ) -> Result<PayoutWithInclusionEstimate, ApplicationError> {
        let payout = self
            .payouts
            .find_by_account_id_and_external_id(profile.account_id, external_id)
            .await?;
        Ok(self
            .batch_inclusion
            .include_estimate(profile.account_id, payout)
            .await?)
    }

    #[instrument(name = "app.find_payout", skip_all, err)]
    pub async fn find_payout(
        &self,
        profile: &Profile,
        id: PayoutId,
    ) -> Result<PayoutWithInclusionEstimate, ApplicationError> {
        let payout = self
            .payouts
            .find_by_account_id_and_id(profile.account_id, id)
            .await?;
        Ok(self
            .batch_inclusion
            .include_estimate(profile.account_id, payout)
            .await?)
    }

    #[instrument(name = "app.list_payouts", skip_all, err)]
    pub async fn list_payouts(
        &self,
        profile: &Profile,
        wallet_name: String,
        page: u64,
        page_size: u64,
    ) -> Result<Vec<PayoutWithInclusionEstimate>, ApplicationError> {
        let wallet = self
            .wallets
            .find_by_account_id_and_name(profile.account_id, wallet_name)
            .await?;
        let payouts = self
            .payouts
            .list_for_wallet(profile.account_id, wallet.id, page, page_size)
            .await?;

        Ok(self
            .batch_inclusion
            .include_estimates(profile.account_id, payouts)
            .await?)
    }

    #[instrument(name = "app.list_payout_queues", skip_all, err)]
    pub async fn list_payout_queues(
        &self,
        profile: &Profile,
    ) -> Result<Vec<PayoutQueue>, ApplicationError> {
        Ok(self
            .payout_queues
            .list_for_account_id(profile.account_id)
            .await?)
    }

    #[instrument(name = "app.update_payout_queue", skip(self), err)]
    pub async fn update_payout_queue(
        &self,
        profile: &Profile,
        id: PayoutQueueId,
        new_description: Option<String>,
        new_config: Option<PayoutQueueConfig>,
    ) -> Result<(), ApplicationError> {
        let mut payout_queue = self
            .payout_queues
            .find_by_account_id_and_id(profile.account_id, id)
            .await?;

        if let Some(desc) = new_description {
            payout_queue.update_description(desc)
        }
        if let Some(config) = new_config {
            payout_queue.update_config(config)
        }
        self.payout_queues.update(&mut payout_queue).await?;
        Ok(())
    }

    #[allow(clippy::type_complexity)]
    #[instrument(name = "app.get_batch", skip_all, err)]
    pub async fn get_batch(
        &self,
        profile: &Profile,
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
        profile: &Profile,
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

    #[instrument(name = "app.spawn_sync_all_wallets", level = "trace", skip_all, err)]
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

    #[instrument(
        name = "app.spawn_process_all_payout_queues",
        level = "trace",
        skip_all,
        err
    )]
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

    #[instrument(
        name = "app.spawn_respawn_all_outbox_handlers",
        level = "trace",
        skip_all,
        err
    )]
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
