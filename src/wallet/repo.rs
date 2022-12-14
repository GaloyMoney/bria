use sqlx::{Pool, Postgres, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::{entity::*, keychain::*};
use crate::{error::*, primitives::*};

#[derive(Debug, Clone)]
pub struct Wallets {
    pool: Pool<Postgres>,
}

impl Wallets {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        account_id: AccountId,
        new_wallet: NewWallet,
    ) -> Result<WalletId, BriaError> {
        let record = sqlx::query!(
            r#"INSERT INTO bria_wallet_keychains (account_id, keychain_cfg)
            VALUES ($1, $2)
            RETURNING (id)"#,
            Uuid::from(account_id),
            serde_json::to_value(new_wallet.keychain)?
        )
        .fetch_one(&mut *tx)
        .await?;
        let record = sqlx::query!(
            r#"INSERT INTO bria_wallets (id, wallet_cfg, account_id, ledger_account_id, dust_ledger_account_id, keychain_id, name)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING (id)"#,
            Uuid::from(new_wallet.id),
            serde_json::to_value(new_wallet.config).expect("Couldn't serialize wallet config"),
            Uuid::from(account_id),
            Uuid::from(new_wallet.id),
            Uuid::from(new_wallet.dust_account_id),
            record.id,
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
            r#"SElECT k.id, wallet_cfg, ledger_account_id, dust_ledger_account_id, a.journal_id, keychain_id, keychain_cfg
                 FROM bria_wallets w
                 JOIN bria_wallet_keychains k ON w.keychain_id = k.id JOIN bria_accounts a ON w.account_id = a.id
                 WHERE w.account_id = $1 AND w.name = $2 ORDER BY w.version DESC"#,
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
            ledger_account_id: first_row.ledger_account_id.into(),
            dust_ledger_account_id: first_row.dust_ledger_account_id.into(),
            keychains,
            config,
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
            r#"SElECT w.id, wallet_cfg, ledger_account_id, dust_ledger_account_id, a.journal_id, keychain_id, keychain_cfg
                 FROM bria_wallets w
                 JOIN bria_wallet_keychains k ON w.keychain_id = k.id JOIN bria_accounts a ON w.account_id = a.id
                 WHERE w.id = $1 ORDER BY w.version DESC"#,
            Uuid::from(id)
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
            ledger_account_id: first_row.ledger_account_id.into(),
            dust_ledger_account_id: first_row.dust_ledger_account_id.into(),
            keychains,
            config,
        };
        for row in iter {
            let keychain: WalletKeyChainConfig = serde_json::from_value(row.keychain_cfg)?;
            wallet.previous_keychain(KeychainId::from(row.keychain_id), keychain);
        }
        Ok(wallet)
    }

    pub async fn list_by_ids(
        &self,
        ids: HashSet<WalletId>,
    ) -> Result<HashMap<WalletId, Wallet>, BriaError> {
        let uuids = ids.into_iter().map(|id| Uuid::from(id)).collect::<Vec<_>>();
        let rows = sqlx::query!(r#"
           SELECT w.id, wallet_cfg, ledger_account_id, dust_ledger_account_id, a.journal_id, keychain_id, keychain_cfg
             FROM bria_wallets w
             JOIN bria_wallet_keychains k ON w.keychain_id = k.id JOIN bria_accounts a ON w.account_id = a.id
             WHERE w.id = ANY($1) ORDER BY w.version DESC"#,
            &uuids[..]
      ).fetch_all(&self.pool).await?;
        let mut wallets = HashMap::new();
        for row in rows {
            let keychain_id = KeychainId::from(row.keychain_id);
            let keychain: WalletKeyChainConfig = serde_json::from_value(row.keychain_cfg)
                .expect("Couldn't deserialize keychain_cfg");
            let wallet = wallets.entry(row.id).or_insert_with(|| Wallet {
                id: row.id.into(),
                journal_id: row.journal_id.into(),
                ledger_account_id: row.ledger_account_id.into(),
                dust_ledger_account_id: row.dust_ledger_account_id.into(),
                keychains: vec![(keychain_id, keychain.clone())],
                config: serde_json::from_value(row.wallet_cfg)
                    .expect("Couldn't deserialize wallet config"),
            });
            wallet.previous_keychain(keychain_id, keychain);
        }
        unimplemented!()
    }
}
