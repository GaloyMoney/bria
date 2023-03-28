use bdk::{wallet::AddressInfo, LocalUtxo};
use sqlx::{Pool, Postgres, Transaction};
use tracing::instrument;

use crate::{
    error::*,
    primitives::{bitcoin::KeychainKind, *},
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

    #[instrument(name = "wallet_utxos.new_bdk_utxo", skip(self, tx))]
    pub async fn new_bdk_utxo(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        address: &AddressInfo,
        utxo: &LocalUtxo,
    ) -> Result<Option<LedgerTransactionId>, BriaError> {
        if let KeychainKind::External = address.keychain {
            let new_utxo = NewWalletUtxo::builder()
                .keychain_id(keychain_id)
                .outpoint(utxo.outpoint)
                .kind(address.keychain)
                .address_idx(address.index)
                .address(address.to_string())
                .script_hex(format!("{:x}", utxo.txout.script_pubkey))
                .value(utxo.txout.value)
                .build()
                .expect("Could not build NewWalletUtxo");
            let ret = new_utxo.ledger_tx_pending_id;
            self.wallet_utxos.persist(tx, new_utxo).await?;
            Ok(Some(ret))
        } else {
            Ok(None)
        }
    }
}
