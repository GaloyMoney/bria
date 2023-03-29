use sqlx::{Pool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::{balance::*, entity::*, keychain::*};
use crate::{error::*, primitives::*};

#[derive(Debug, Clone)]
pub struct Wallets {
    pool: Pool<Postgres>,
    network: bitcoin::Network,
}

impl Wallets {
    pub fn new(pool: &Pool<Postgres>, network: bitcoin::Network) -> Self {
        Self {
            pool: pool.clone(),
            network,
        }
    }

    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: AccountId,
        new_wallet: NewWallet,
    ) -> Result<WalletId, BriaError> {
        sqlx::query!(
            r#"INSERT INTO bria_wallet_keychains (account_id, wallet_id, keychain_cfg)
            VALUES ($1, $2, $3)"#,
            Uuid::from(account_id),
            Uuid::from(new_wallet.id),
            serde_json::to_value(new_wallet.keychain)?
        )
        .execute(&mut *tx)
        .await?;
        let record = sqlx::query!(
            r#"INSERT INTO bria_wallets (id, wallet_cfg, account_id, incoming_ledger_account_id, at_rest_ledger_account_id, fee_ledger_account_id, outgoing_ledger_account_id, dust_ledger_account_id, name)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING (id)"#,
            Uuid::from(new_wallet.id),
            serde_json::to_value(new_wallet.config).expect("Couldn't serialize wallet config"),
            Uuid::from(account_id),
            Uuid::from(new_wallet.ledger_account_ids.onchain_incoming_id),
            Uuid::from(new_wallet.ledger_account_ids.onchain_at_rest_id),
            Uuid::from(new_wallet.ledger_account_ids.fee_id),
            Uuid::from(new_wallet.ledger_account_ids.onchain_outgoing_id),
            Uuid::from(new_wallet.ledger_account_ids.dust_id),
            new_wallet.name
        )
        .fetch_one(&mut *tx)
        .await?;
        Ok(WalletId::from(record.id))
    }

    pub async fn find_by_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<Wallet, BriaError> {
        let rows = sqlx::query!(
            r#"WITH latest AS (
              SELECT w.id, w.wallet_cfg, w.incoming_ledger_account_id, w.at_rest_ledger_account_id, w.fee_ledger_account_id, w.outgoing_ledger_account_id, w.dust_ledger_account_id, a.journal_id 
              FROM bria_wallets w JOIN bria_accounts a ON w.account_id = a.id
              WHERE a.id = $1 AND w.name = $2 ORDER BY version DESC LIMIT 1
            )
            SELECT l.id, l.wallet_cfg, l.incoming_ledger_account_id, l.at_rest_ledger_account_id, l.fee_ledger_account_id, l.outgoing_ledger_account_id, l.dust_ledger_account_id, l.journal_id, k.id AS keychain_id, keychain_cfg
                 FROM bria_wallet_keychains k
                 JOIN latest l ON k.wallet_id = l.id
                 ORDER BY sequence DESC"#,
            Uuid::from(account_id),
            name
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Err(BriaError::WalletNotFound);
        }
        let mut iter = rows.into_iter();
        let first_row = iter.next().expect("There is always 1 row here");
        let keychain: WalletKeyChainConfig = serde_json::from_value(first_row.keychain_cfg)?;
        let mut keychains = vec![(KeychainId::from(first_row.keychain_id), keychain)];
        let mut config: WalletConfig = serde_json::from_value(first_row.wallet_cfg)?;
        for row in iter {
            let keychain: WalletKeyChainConfig = serde_json::from_value(row.keychain_cfg)?;
            keychains.push((KeychainId::from(row.keychain_id), keychain));
            config = serde_json::from_value(row.wallet_cfg)?;
        }
        Ok(Wallet {
            id: first_row.id.into(),
            journal_id: first_row.journal_id.into(),
            ledger_account_ids: WalletLedgerAccountIds {
                onchain_incoming_id: first_row.incoming_ledger_account_id.into(),
                onchain_at_rest_id: first_row.at_rest_ledger_account_id.into(),
                fee_id: first_row.fee_ledger_account_id.into(),
                onchain_outgoing_id: first_row.outgoing_ledger_account_id.into(),
                dust_id: first_row.dust_ledger_account_id.into(),
            },
            keychains,
            config,
            network: self.network,
        })
    }

    pub async fn all_ids(&self) -> Result<impl Iterator<Item = WalletId>, BriaError> {
        let rows = sqlx::query!(r#"SELECT distinct(id) FROM bria_wallets"#,)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|row| WalletId::from(row.id)))
    }

    pub async fn find_by_id(&self, id: WalletId) -> Result<Wallet, BriaError> {
        let rows = sqlx::query!(
            r#"WITH latest AS (
              SELECT w.id, w.wallet_cfg, w.incoming_ledger_account_id, w.at_rest_ledger_account_id, w.fee_ledger_account_id, w.outgoing_ledger_account_id, w.dust_ledger_account_id, a.journal_id 
              FROM bria_wallets w JOIN bria_accounts a ON w.account_id = a.id
              WHERE w.id = $1 ORDER BY version DESC LIMIT 1
            )
            SELECT l.id, l.wallet_cfg, l.incoming_ledger_account_id, l.at_rest_ledger_account_id, l.fee_ledger_account_id, l.outgoing_ledger_account_id, l.dust_ledger_account_id, l.journal_id, k.id AS keychain_id, keychain_cfg
                 FROM bria_wallet_keychains k
                 JOIN latest l ON k.wallet_id = l.id
                 ORDER BY sequence DESC"#,
            Uuid::from(id),
        )
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Err(BriaError::WalletNotFound);
        }
        let mut iter = rows.into_iter();
        let first_row = iter.next().expect("There is always 1 row here");
        let keychain: WalletKeyChainConfig = serde_json::from_value(first_row.keychain_cfg)?;
        let keychains = vec![(KeychainId::from(first_row.keychain_id), keychain)];
        let config: WalletConfig = serde_json::from_value(first_row.wallet_cfg)?;
        let mut wallet = Wallet {
            id: first_row.id.into(),
            journal_id: first_row.journal_id.into(),
            ledger_account_ids: WalletLedgerAccountIds {
                onchain_incoming_id: first_row.incoming_ledger_account_id.into(),
                onchain_at_rest_id: first_row.at_rest_ledger_account_id.into(),
                fee_id: first_row.fee_ledger_account_id.into(),
                onchain_outgoing_id: first_row.outgoing_ledger_account_id.into(),
                dust_id: first_row.dust_ledger_account_id.into(),
            },
            keychains,
            config,
            network: self.network,
        };
        for row in iter {
            let keychain: WalletKeyChainConfig = serde_json::from_value(row.keychain_cfg)?;
            wallet.previous_keychain(KeychainId::from(row.keychain_id), keychain);
        }
        Ok(wallet)
    }

    pub async fn find_by_ids(
        &self,
        ids: HashSet<WalletId>,
    ) -> Result<HashMap<WalletId, Wallet>, BriaError> {
        let uuids = ids.into_iter().map(Uuid::from).collect::<Vec<_>>();
        let rows = sqlx::query!(
            r#"WITH latest AS (
              SELECT w.id, w.wallet_cfg, w.incoming_ledger_account_id, w.at_rest_ledger_account_id, w.fee_ledger_account_id, w.outgoing_ledger_account_id, w.dust_ledger_account_id, a.journal_id
              FROM bria_wallets w JOIN bria_accounts a ON w.account_id = a.id
              WHERE w.id = ANY($1) ORDER BY version DESC LIMIT 1
            )
            SELECT l.id, l.wallet_cfg, l.incoming_ledger_account_id, l.at_rest_ledger_account_id, l.fee_ledger_account_id, l.outgoing_ledger_account_id, l.dust_ledger_account_id, l.journal_id, k.id AS keychain_id, keychain_cfg
                 FROM bria_wallet_keychains k
                 JOIN latest l ON k.wallet_id = l.id
                 ORDER BY sequence DESC"#,
            &uuids[..]
      ).fetch_all(&self.pool).await?;
        let mut wallets = HashMap::new();
        for row in rows {
            let keychain_id = KeychainId::from(row.keychain_id);
            let keychain: WalletKeyChainConfig = serde_json::from_value(row.keychain_cfg)
                .expect("Couldn't deserialize keychain_cfg");
            let wallet = wallets
                .entry(WalletId::from(row.id))
                .or_insert_with(|| Wallet {
                    id: row.id.into(),
                    journal_id: row.journal_id.into(),
                    ledger_account_ids: WalletLedgerAccountIds {
                        onchain_incoming_id: row.incoming_ledger_account_id.into(),
                        onchain_at_rest_id: row.at_rest_ledger_account_id.into(),
                        fee_id: row.fee_ledger_account_id.into(),
                        onchain_outgoing_id: row.outgoing_ledger_account_id.into(),
                        dust_id: row.dust_ledger_account_id.into(),
                    },
                    keychains: vec![(keychain_id, keychain.clone())],
                    config: serde_json::from_value(row.wallet_cfg)
                        .expect("Couldn't deserialize wallet config"),
                    network: self.network,
                });
            wallet.previous_keychain(keychain_id, keychain);
        }
        Ok(wallets)
    }
}
