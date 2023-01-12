use bdk::LocalUtxo;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use sqlx_ledger::{AccountId as LedgerAccountId, JournalId};

use super::{balance::WalletLedgerAccountIds, keychain::*};
use crate::primitives::*;

pub struct Wallet {
    pub id: WalletId,
    pub ledger_account_ids: WalletLedgerAccountIds,
    pub journal_id: JournalId,
    pub keychains: Vec<(KeychainId, WalletKeyChainConfig)>,
    pub config: WalletConfig,
    pub network: bitcoin::Network,
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

    pub fn is_dust_utxo(&self, utxo: &LocalUtxo) -> bool {
        utxo.txout.value <= self.config.dust_threshold_sats
    }

    pub fn pick_dust_or_ledger_account(
        &self,
        utxo: &LocalUtxo,
        account: LedgerAccountId,
    ) -> LedgerAccountId {
        if self.is_dust_utxo(&utxo) {
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
    pub mark_settled_after_n_confs: u32,
    pub dust_threshold_sats: u64,
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            mark_settled_after_n_confs: 2,
            dust_threshold_sats: 0,
        }
    }
}
