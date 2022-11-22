use bdk::{
    descriptor::IntoWalletDescriptor,
    wallet::{AddressIndex, AddressInfo},
    Wallet,
};
use bitcoin::Network;
use sqlx::PgPool;

use super::pg::SqlxWalletDb;
use crate::primitives::*;

pub struct BdkWallet {
    pool: PgPool,
    network: Network,
    keychain_id: KeychainId,
    descriptor: String,
}

impl BdkWallet {
    pub fn new(
        pool: PgPool,
        network: Network,
        keychain_id: KeychainId,
        descriptor: String,
    ) -> Self {
        Self {
            pool,
            network,
            keychain_id,
            descriptor,
        }
    }

    pub async fn next_address(&self) -> Result<bdk::wallet::AddressInfo, tokio::task::JoinError> {
        let addr = self
            .with_wallet(|wallet| {
                wallet
                    .get_address(AddressIndex::New)
                    .expect("Couldn't get new address")
            })
            .await?;
        Ok(addr)
    }

    async fn with_wallet<F, T>(&self, f: F) -> Result<T, tokio::task::JoinError>
    where
        F: 'static + Send + FnOnce(Wallet<SqlxWalletDb>) -> T,
        T: Send + 'static,
    {
        let descriptor = self.descriptor.clone();
        let pool = self.pool.clone();
        let keychain_id = self.keychain_id.clone();
        let network = self.network.clone();
        let res = tokio::task::spawn_blocking(move || {
            let wallet = Wallet::new(
                descriptor.as_str(),
                None,
                network,
                SqlxWalletDb::new(pool, keychain_id),
            )
            .expect("Couldn't construct wallet");
            f(wallet)
        })
        .await?;
        Ok(res)
    }
}
