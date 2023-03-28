use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use std::collections::HashMap;

use super::entity::*;
use crate::{
    error::*,
    primitives::{bitcoin::*, *},
};

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
            r#"INSERT INTO bria_wallet_utxos
               (wallet_id, keychain_id, tx_id, vout, kind, address_idx, value, address, script_hex, spent)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
               Uuid::from(utxo.wallet_id),
               Uuid::from(utxo.keychain_id),
               utxo.outpoint.txid.to_string(),
               utxo.outpoint.vout as i32,
               pg::PgKeychainKind::from(utxo.kind) as pg::PgKeychainKind,
               utxo.address_idx as i32,
               utxo.value.into_inner(),
               utxo.address,
               utxo.script_hex,
               utxo.spent,
        )
            .execute(&mut *tx)
            .await?;
        Ok(())
    }

    pub async fn find_keychain_utxos(
        &self,
        keychain_ids: impl Iterator<Item = KeychainId>,
    ) -> Result<HashMap<KeychainId, KeychainUtxos>, BriaError> {
        let keychain_ids: Vec<Uuid> = keychain_ids.map(Uuid::from).collect();
        let rows = sqlx::query!(
            r#"SELECT wallet_id, keychain_id, tx_id, vout, kind as "kind: pg::PgKeychainKind", address_idx, value, address, spent as spent,
                  CASE
                      WHEN kind = 'external' THEN address
                      ELSE NULL
                  END as optional_address,
                  block_height
           FROM bria_wallet_utxos
           WHERE keychain_id = ANY($1) AND spent = false
           ORDER BY created_at DESC"#,
           &keychain_ids
        )
            .fetch_all(&self._pool)
            .await?;

        let mut utxos = HashMap::new();

        for row in rows {
            let utxo = WalletUtxo {
                wallet_id: row.wallet_id.into(),
                keychain_id: row.keychain_id.into(),
                outpoint: OutPoint {
                    txid: row.tx_id.parse().unwrap(),
                    vout: row.vout as u32,
                },
                kind: KeychainKind::from(row.kind),
                address_idx: row.address_idx as u32,
                value: Satoshis::from(row.value),
                address: row.optional_address,
                spent: row.spent,
                block_height: row.block_height.map(|v| v as u32),
            };

            let keychain_id = KeychainId::from(row.keychain_id);
            utxos
                .entry(keychain_id)
                .or_insert_with(|| KeychainUtxos {
                    keychain_id,
                    utxos: Vec::new(),
                })
                .utxos
                .push(utxo);
        }

        Ok(utxos)
    }
}
