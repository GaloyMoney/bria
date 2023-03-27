use bdk::{wallet::AddressInfo, LocalUtxo};
use sqlx::{Pool, Postgres, Transaction};

use crate::{error::*, ledger::*, primitives::*};

use super::entity::*;

#[derive(Clone)]
pub struct WalletUtxos {
    pool: Pool<Postgres>,
    ledger: Ledger,
}

impl WalletUtxos {
    pub fn new(pool: &Pool<Postgres>, ledger: &Ledger) -> Self {
        Self {
            pool: pool.clone(),
            ledger: ledger.clone(),
        }
    }

    pub async fn new_bdk_utxo(
        &self,
        _tx: Transaction<'_, Postgres>,
        _keychain_id: KeychainId,
        _address: AddressInfo,
        _utxo: LocalUtxo,
    ) -> Result<(), BriaError> {
        // find address via
        // scriptpubkeys.findpath
        // keychainwallet.lookup address
        // psbt builder moves to WalletUtxos
        Ok(())
    }
}
