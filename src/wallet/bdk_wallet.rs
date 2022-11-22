use bdk::{wallet::AddressIndex, Wallet};
use bitcoin::Network;
use sqlx::PgPool;

use crate::bdk::pg::SqlxWalletDb;
use crate::primitives::*;

pub trait IntoExternalDescriptor {
    fn into_external_descriptor(self) -> String;
}
impl IntoExternalDescriptor for String {
    fn into_external_descriptor(self) -> String {
        self
    }
}

pub trait IntoInternalDescriptor {
    fn into_internal_descriptor(self) -> String;
}

pub struct BdkWallet<E> {
    pool: PgPool,
    network: Network,
    keychain_id: KeychainId,
    external_descriptor: E,
}

impl<E: IntoExternalDescriptor + Clone + Send + Sync + 'static> BdkWallet<E> {
    pub fn new(
        pool: PgPool,
        network: Network,
        keychain_id: KeychainId,
        external_descriptor: E,
    ) -> Self {
        Self {
            pool,
            network,
            keychain_id,
            external_descriptor,
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
        let external_descriptor = self.external_descriptor.clone();
        let pool = self.pool.clone();
        let keychain_id = self.keychain_id;
        let network = self.network;
        let res = tokio::task::spawn_blocking(move || {
            let wallet = Wallet::new(
                external_descriptor.into_external_descriptor().as_str(),
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
