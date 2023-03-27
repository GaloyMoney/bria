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
        tx: Transaction<'_, Postgres>,
        keychain_id: KeychainId,
        address: AddressInfo,
        utxo: LocalUtxo,
    ) -> Result<(), BriaError> {
        // ledger
        //     .incoming_utxo(
        //         tx,
        //         pending_id,
        //         IncomingUtxoParams {
        //             journal_id: wallet.journal_id,
        //             ledger_account_incoming_id: wallet.pick_dust_or_ledger_account(
        //                 &local_utxo,
        //                 wallet.ledger_account_ids.incoming_id,
        //             ),
        //             meta: IncomingUtxoMeta {
        //                 wallet_id: data.wallet_id,
        //                 keychain_id,
        //                 outpoint: local_utxo.outpoint,
        //                 txout: local_utxo.txout,
        //                 confirmation_time,
        //             },
        //         },
        //     )
        //     .await?;
        Ok(())
    }
}
