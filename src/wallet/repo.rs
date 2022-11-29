use sqlx::{Pool, Postgres, Transaction};
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
            r#"INSERT INTO keychains (account_id, config)
            VALUES ($1, $2)
            RETURNING (id)"#,
            Uuid::from(account_id),
            serde_json::to_value(new_wallet.keychain)?
        )
        .fetch_one(&mut *tx)
        .await?;
        let record = sqlx::query!(
            r#"INSERT INTO wallets (id, account_id, ledger_account_id, dust_ledger_account_id, keychain_id, name)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING (id)"#,
            Uuid::from(new_wallet.id),
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
            r#"SElECT k.id, ledger_account_id, dust_ledger_account_id, keychain_id, config
                 FROM wallets w
                 JOIN keychains k ON w.keychain_id = k.id
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
        let keychain: WalletKeyChainConfig =
            serde_json::from_value(first_row.config.expect("Should always have config"))?;
        let mut keychains = vec![(KeychainId::from(first_row.keychain_id), keychain)];
        for row in iter {
            let keychain: WalletKeyChainConfig =
                serde_json::from_value(row.config.expect("Should always have config"))?;
            keychains.push((KeychainId::from(row.keychain_id), keychain));
        }
        Ok(Wallet {
            id: first_row.id.into(),
            ledger_account_id: first_row.ledger_account_id.into(),
            dust_ledger_account_id: first_row.dust_ledger_account_id.into(),
            keychains,
        })
    }

    pub async fn all_keychain_ids(&self) -> Result<Vec<KeychainId>, BriaError> {
        let rows = sqlx::query!(r#"SELECT id FROM keychains"#,)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|row| row.id.into()).collect())
    }
}
