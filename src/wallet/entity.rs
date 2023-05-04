use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{AccountId as LedgerAccountId, JournalId};

use std::collections::HashMap;

use super::{balance::WalletLedgerAccountIds, config::*, keychain::*};
use crate::{entity::*, primitives::*, xpub::XPub};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WalletEvent {
    WalletInitialized {
        id: WalletId,
        network: bitcoin::Network,
        account_id: AccountId,
        journal_id: JournalId,
        onchain_incoming_ledger_account_id: LedgerAccountId,
        onchain_at_rest_ledger_account_id: LedgerAccountId,
        onchain_outgoing_ledger_account_id: LedgerAccountId,
        onchain_fee_ledger_account_id: LedgerAccountId,
        logical_incoming_ledger_account_id: LedgerAccountId,
        logical_at_rest_ledger_account_id: LedgerAccountId,
        logical_outgoing_ledger_account_id: LedgerAccountId,
        dust_ledger_account_id: LedgerAccountId,
    },
    WalletNameUpdated {
        name: String,
    },
    WalletConfigUpdated {
        config: WalletConfig,
    },
    WalletKeychainAdded {
        keychain_id: KeychainId,
        idx: usize,
        config: WalletKeyChainConfig,
    },
}

#[derive(Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct Wallet {
    pub id: WalletId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub journal_id: JournalId,
    pub config: WalletConfig,
    pub network: bitcoin::Network,
    pub name: String,

    events: EntityEvents<WalletEvent>,
}

impl Wallet {
    fn iter_keychains(&self) -> impl Iterator<Item = (&KeychainId, &WalletKeyChainConfig)> + '_ {
        self.events.iter().rev().filter_map(|e| {
            if let WalletEvent::WalletKeychainAdded {
                keychain_id,
                config,
                ..
            } = e
            {
                Some((keychain_id, config))
            } else {
                None
            }
        })
    }

    pub fn keychain_ids(&self) -> impl Iterator<Item = KeychainId> + '_ {
        self.iter_keychains().map(|(id, _)| *id)
    }

    pub fn keychain_wallets(
        &self,
        pool: sqlx::PgPool,
    ) -> impl Iterator<Item = KeychainWallet<WalletKeyChainConfig>> + '_ {
        let current = self.current_keychain_wallet(&pool);
        std::iter::once(current).chain(self.deprecated_keychain_wallets(pool))
    }

    pub fn current_keychain_wallet(
        &self,
        pool: &sqlx::PgPool,
    ) -> KeychainWallet<WalletKeyChainConfig> {
        let (id, cfg) = self.iter_keychains().next().expect("No current keychain");
        KeychainWallet::new(pool.clone(), self.network, *id, cfg.clone())
    }

    pub fn deprecated_keychain_wallets(
        &self,
        pool: sqlx::PgPool,
    ) -> impl Iterator<Item = KeychainWallet<WalletKeyChainConfig>> + '_ {
        self.iter_keychains()
            .skip(1)
            .map(move |(id, cfg)| KeychainWallet::new(pool.clone(), self.network, *id, cfg.clone()))
    }

    pub fn xpubs_for_keychains<'a>(
        &self,
        keychain_ids: impl IntoIterator<Item = &'a KeychainId>,
    ) -> HashMap<KeychainId, Vec<XPub>> {
        let mut ret = HashMap::new();
        for find_id in keychain_ids {
            if let Some((_, cfg)) = self.iter_keychains().find(|(id, _)| id == &find_id) {
                ret.insert(*find_id, cfg.xpubs());
            }
        }
        ret
    }
}

#[derive(Builder, Clone)]
pub struct NewWallet {
    #[builder(setter(into))]
    pub id: WalletId,
    pub(super) network: bitcoin::Network,
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) journal_id: JournalId,
    pub(super) ledger_account_ids: WalletLedgerAccountIds,
    pub(super) name: String,
    #[builder(setter(into))]
    keychain: WalletKeyChainConfig,
    #[builder(default)]
    config: WalletConfig,
}

impl NewWallet {
    pub fn builder() -> NewWalletBuilder {
        let mut builder = NewWalletBuilder::default();
        builder.id(WalletId::new());
        builder
    }

    pub(super) fn initial_events(self) -> EntityEvents<WalletEvent> {
        EntityEvents::init([
            WalletEvent::WalletInitialized {
                id: self.id,
                network: self.network,
                account_id: self.account_id,
                journal_id: self.journal_id,
                onchain_incoming_ledger_account_id: self.ledger_account_ids.onchain_incoming_id,
                onchain_at_rest_ledger_account_id: self.ledger_account_ids.onchain_at_rest_id,
                onchain_outgoing_ledger_account_id: self.ledger_account_ids.onchain_outgoing_id,
                onchain_fee_ledger_account_id: self.ledger_account_ids.fee_id,
                logical_incoming_ledger_account_id: self.ledger_account_ids.logical_incoming_id,
                logical_at_rest_ledger_account_id: self.ledger_account_ids.logical_at_rest_id,
                logical_outgoing_ledger_account_id: self.ledger_account_ids.logical_outgoing_id,
                dust_ledger_account_id: self.ledger_account_ids.dust_id,
            },
            WalletEvent::WalletNameUpdated { name: self.name },
            WalletEvent::WalletConfigUpdated {
                config: self.config,
            },
            WalletEvent::WalletKeychainAdded {
                keychain_id: KeychainId::new(),
                idx: 0,
                config: self.keychain,
            },
        ])
    }
}

impl TryFrom<EntityEvents<WalletEvent>> for Wallet {
    type Error = EntityError;

    fn try_from(events: EntityEvents<WalletEvent>) -> Result<Self, Self::Error> {
        let mut builder = WalletBuilder::default();
        use WalletEvent::*;
        for event in events.iter() {
            match event {
                WalletInitialized {
                    id,
                    network,
                    journal_id,
                    onchain_incoming_ledger_account_id,
                    onchain_at_rest_ledger_account_id,
                    onchain_outgoing_ledger_account_id,
                    onchain_fee_ledger_account_id,
                    logical_incoming_ledger_account_id,
                    logical_at_rest_ledger_account_id,
                    logical_outgoing_ledger_account_id,
                    dust_ledger_account_id,
                    ..
                } => {
                    builder = builder
                        .id(*id)
                        .network(*network)
                        .journal_id(*journal_id)
                        .ledger_account_ids(WalletLedgerAccountIds {
                            onchain_incoming_id: *onchain_incoming_ledger_account_id,
                            onchain_at_rest_id: *onchain_at_rest_ledger_account_id,
                            onchain_outgoing_id: *onchain_outgoing_ledger_account_id,
                            fee_id: *onchain_fee_ledger_account_id,
                            logical_incoming_id: *logical_incoming_ledger_account_id,
                            logical_at_rest_id: *logical_at_rest_ledger_account_id,
                            logical_outgoing_id: *logical_outgoing_ledger_account_id,
                            dust_id: *dust_ledger_account_id,
                        });
                }
                WalletConfigUpdated { config } => {
                    builder = builder.config(config.clone());
                }
                WalletNameUpdated { name } => {
                    builder = builder.name(name.clone());
                }
                _ => (),
            }
        }
        builder.events(events).build()
    }
}
