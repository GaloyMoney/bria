use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{AccountId as LedgerAccountId, JournalId};

use super::{balance::WalletLedgerAccountIds, keychain::*};
use crate::primitives::*;

pub struct Wallet {
    pub id: WalletId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub journal_id: JournalId,
    pub config: WalletConfig,
    pub network: bitcoin::Network,

    pub(super) keychains: Vec<(KeychainId, WalletKeyChainConfig)>,
}

impl Wallet {
    pub(super) fn previous_keychain(&mut self, id: KeychainId, cfg: WalletKeyChainConfig) {
        let last_id = self.keychains[self.keychains.len() - 1].0;
        if id != last_id {
            self.keychains.push((id, cfg));
        }
    }
}

impl Wallet {
    pub fn keychain_ids(&self) -> impl Iterator<Item = KeychainId> + '_ {
        self.keychains.iter().map(|(id, _)| *id)
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
        let (id, cfg) = &self.keychains[0];
        KeychainWallet::new(pool.clone(), self.network, *id, cfg.clone())
    }

    pub fn deprecated_keychain_wallets(
        &self,
        pool: sqlx::PgPool,
    ) -> impl Iterator<Item = KeychainWallet<WalletKeyChainConfig>> + '_ {
        self.keychains
            .iter()
            .skip(1)
            .map(move |(id, cfg)| KeychainWallet::new(pool.clone(), self.network, *id, cfg.clone()))
    }

    pub fn is_dust_utxo(&self, value: Satoshis) -> bool {
        value <= self.config.dust_threshold_sats
    }

    pub fn pick_dust_or_ledger_account(
        &self,
        value: Satoshis,
        account: LedgerAccountId,
    ) -> LedgerAccountId {
        if self.is_dust_utxo(value) {
            self.ledger_account_ids.dust_id
        } else {
            account
        }
    }
}

#[derive(Builder, Clone)]
pub struct NewWallet {
    #[builder(setter(into))]
    pub id: WalletId,
    pub(super) name: String,
    #[builder(setter(into))]
    pub(super) keychain: WalletKeyChainConfig,
    pub(super) ledger_account_ids: WalletLedgerAccountIds,
    #[builder(default)]
    pub(super) config: WalletConfig,
}

impl NewWallet {
    pub fn builder() -> NewWalletBuilder {
        let mut builder = NewWalletBuilder::default();
        builder.id(WalletId::new());
        builder
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    settle_income_after_n_confs: u32,
    settle_change_after_n_confs: u32,
    pub dust_threshold_sats: Satoshis,
}

impl WalletConfig {
    pub fn latest_income_settle_height(&self, current_height: u32) -> u32 {
        current_height - self.settle_income_after_n_confs.max(1) + 1
    }

    pub fn latest_change_settle_height(&self, current_height: u32) -> u32 {
        current_height - self.settle_change_after_n_confs.max(1) + 1
    }

    pub fn latest_settle_height(&self, current_height: u32, self_pay: bool) -> u32 {
        if self_pay {
            self.latest_change_settle_height(current_height)
        } else {
            self.latest_income_settle_height(current_height)
        }
    }
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            settle_income_after_n_confs: 2,
            settle_change_after_n_confs: 1,
            dust_threshold_sats: Satoshis::from(0),
        }
    }
}
