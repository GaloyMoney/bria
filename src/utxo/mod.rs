mod entity;
mod repo;

use bdk::{wallet::AddressInfo, LocalUtxo};
use sqlx::{Pool, Postgres, Transaction};
use tracing::instrument;

use std::collections::HashMap;

use crate::{
    error::*,
    primitives::{bitcoin::OutPoint, *},
};
pub use entity::*;
use repo::*;

#[derive(Clone)]
pub struct Utxos {
    utxos: UtxoRepo,
}

impl Utxos {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self {
            utxos: UtxoRepo::new(pool.clone()),
        }
    }

    #[instrument(name = "utxos.new_utxo", skip(self))]
    pub async fn new_utxo(
        &self,
        wallet_id: WalletId,
        keychain_id: KeychainId,
        address: &AddressInfo,
        utxo: &LocalUtxo,
        sats_per_vbyte_when_created: f32,
        self_pay: bool,
    ) -> Result<Option<(LedgerTransactionId, Transaction<'_, Postgres>)>, BriaError> {
        let new_utxo = NewUtxo::builder()
            .wallet_id(wallet_id)
            .keychain_id(keychain_id)
            .outpoint(utxo.outpoint)
            .kind(address.keychain)
            .address_idx(address.index)
            .address(address.to_string())
            .spent(utxo.is_spent)
            .script_hex(format!("{:x}", utxo.txout.script_pubkey))
            .value(utxo.txout.value)
            .sats_per_vbyte_when_created(sats_per_vbyte_when_created)
            .self_pay(self_pay)
            .build()
            .expect("Could not build NewUtxo");
        self.utxos.persist_utxo(new_utxo).await
    }

    #[instrument(name = "utxos.confirm_utxo", skip(self, tx))]
    pub async fn confirm_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        outpoint: OutPoint,
        spent: bool,
        block_height: u32,
    ) -> Result<ConfirmedUtxo, BriaError> {
        self.utxos
            .mark_utxo_confirmed(tx, keychain_id, outpoint, spent, block_height)
            .await
    }

    #[instrument(name = "utxos.find_keychain_utxos", skip_all)]
    pub async fn find_keychain_utxos(
        &self,
        keychain_ids: impl Iterator<Item = KeychainId>,
    ) -> Result<HashMap<KeychainId, KeychainUtxos>, BriaError> {
        self.utxos.find_keychain_utxos(keychain_ids).await
    }

    #[instrument(name = "utxos.outpoints_bdk_should_not_select", skip_all)]
    pub async fn outpoints_bdk_should_not_select(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ids: impl Iterator<Item = KeychainId>,
    ) -> Result<HashMap<KeychainId, Vec<OutPoint>>, BriaError> {
        // Here we list all Utxos that bdk might want to use and lock them (FOR UPDATE)
        // This ensures that we don't have 2 concurrent psbt constructions get in the way
        // of each other
        let reservable_utxos = self.utxos.find_reservable_utxos(tx, ids).await?;

        // We need to tell bdk which utxos not to select.
        // If we have included it in a batch OR
        // it is an income address and not recorded as settled yet
        // we need to flag it to bdk
        let filtered_utxos = reservable_utxos.into_iter().filter_map(|utxo| {
            if utxo.spending_batch_id.is_some()
                || (utxo.income_address && utxo.confirmed_ledger_tx_id.is_none())
            {
                Some((utxo.keychain_id, utxo.outpoint))
            } else {
                None
            }
        });

        let mut outpoints_map = HashMap::new();
        for (keychain_id, outpoint) in filtered_utxos {
            outpoints_map
                .entry(keychain_id)
                .or_insert_with(Vec::new)
                .push(outpoint);
        }

        Ok(outpoints_map)
    }

    #[instrument(name = "utxos.reserve_utxos_in_batch", skip_all)]
    pub async fn reserve_utxos_in_batch(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        batch_id: BatchId,
        utxos: impl Iterator<Item = (KeychainId, OutPoint)>,
    ) -> Result<(), BriaError> {
        self.utxos.reserve_utxos_in_batch(tx, batch_id, utxos).await
    }

    #[instrument(name = "utxos.get_pending_ledger_tx_ids_for_utxos", skip(self))]
    pub async fn get_pending_ledger_tx_ids_for_utxos(
        &self,
        utxos: &HashMap<KeychainId, Vec<OutPoint>>,
    ) -> Result<Vec<LedgerTransactionId>, BriaError> {
        self.utxos.get_pending_ledger_tx_ids_for_utxos(utxos).await
    }
}
