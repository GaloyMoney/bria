use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use super::entity::*;
use crate::{error::*, primitives::bitcoin::pg};

#[derive(Clone)]
pub(super) struct WalletUtxoRepo {
    _pool: Pool<Postgres>,
}

impl WalletUtxoRepo {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { _pool: pool }
    }

    pub async fn persist(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        utxo: NewWalletUtxo,
    ) -> Result<(), BriaError> {
        sqlx::query!(
            r#"INSERT INTO bria_wallet_utxos (keychain_id, tx_id, vout, kind, address_idx, value, address, script_hex, bdk_spent)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
            Uuid::from(utxo.keychain_id),
            utxo.outpoint.txid.to_string(),
            utxo.outpoint.vout as i32,
            pg::PgKeychainKind::from(utxo.kind) as pg::PgKeychainKind,
            utxo.address_idx as i32,
            utxo.value.into_inner(),
            utxo.address,
            utxo.script_hex,
            utxo.bdk_spent,
        )
        .execute(&mut *tx)
        .await?;
        Ok(())
    }
}
