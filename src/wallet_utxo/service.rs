use bdk::{wallet::AddressInfo, LocalUtxo};
use sqlx::{Pool, Postgres, Transaction};
use tracing::instrument;

use std::collections::HashMap;

use crate::{
    error::*,
    primitives::{bitcoin::OutPoint, *},
};

use super::{entity::*, repo::*};

#[derive(Clone)]
pub struct WalletUtxos {
    wallet_utxos: WalletUtxoRepo,
}

impl WalletUtxos {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self {
            wallet_utxos: WalletUtxoRepo::new(pool.clone()),
        }
    }

    #[instrument(name = "wallet_utxos.new_income_utxo", skip(self, tx))]
    pub async fn new_income_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        wallet_id: WalletId,
        keychain_id: KeychainId,
        address: &AddressInfo,
        utxo: &LocalUtxo,
    ) -> Result<LedgerTransactionId, BriaError> {
        let new_utxo = NewWalletUtxo::builder()
            .wallet_id(wallet_id)
            .keychain_id(keychain_id)
            .outpoint(utxo.outpoint)
            .kind(address.keychain)
            .address_idx(address.index)
            .address(address.to_string())
            .spent(utxo.is_spent)
            .script_hex(format!("{:x}", utxo.txout.script_pubkey))
            .value(utxo.txout.value)
            .build()
            .expect("Could not build NewWalletUtxo");
        let ret = new_utxo.pending_ledger_tx_id;
        self.wallet_utxos.persist_income_utxo(tx, new_utxo).await?;
        Ok(ret)
    }

    #[instrument(name = "wallet_utxos.confirm_income_utxo", skip(self, tx))]
    pub async fn confirm_income_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        outpoint: OutPoint,
        spent: bool,
        block_height: u32,
    ) -> Result<ConfimedIncomeUtxo, BriaError> {
        self.wallet_utxos
            .confirm_income_utxo(tx, keychain_id, outpoint, spent, block_height)
            .await
    }

    #[instrument(name = "wallet_utxos.list_utxos_for_wallet", skip_all)]
    pub async fn find_keychain_utxos(
        &self,
        keychain_ids: impl Iterator<Item = KeychainId>,
    ) -> Result<HashMap<KeychainId, KeychainUtxos>, BriaError> {
        self.wallet_utxos.find_keychain_utxos(keychain_ids).await
    }
}
