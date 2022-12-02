use bdk::{
    blockchain::{GetHeight, WalletSync},
    wallet::AddressIndex,
    Wallet,
};
use bitcoin::Network;
use sqlx::PgPool;
use tracing::instrument;

use crate::{bdk::pg::SqlxWalletDb, error::*, primitives::*};

pub trait ToExternalDescriptor {
    fn to_external_descriptor(&self) -> String;
}

pub trait ToInternalDescriptor {
    fn to_internal_descriptor(&self) -> String;
}

pub struct KeychainWallet<T> {
    pool: PgPool,
    network: Network,
    keychain_id: KeychainId,
    descriptor: T,
}

impl<T: ToInternalDescriptor + ToExternalDescriptor + Clone + Send + Sync + 'static>
    KeychainWallet<T>
{
    pub fn new(pool: PgPool, network: Network, keychain_id: KeychainId, descriptor: T) -> Self {
        Self {
            pool,
            network,
            keychain_id,
            descriptor,
        }
    }

    #[instrument(name = "keychain_wallet.new_external_address", skip_all)]
    pub async fn new_external_address(&self) -> Result<bdk::wallet::AddressInfo, BriaError> {
        let addr = self
            .with_wallet(|wallet| {
                wallet
                    .get_address(AddressIndex::New)
                    .expect("Couldn't get new address")
            })
            .await?;
        Ok(addr)
    }

    #[instrument(name = "keychain_wallet.new_internal_address", skip_all)]
    pub async fn new_internal_address(&self) -> Result<bdk::wallet::AddressInfo, BriaError> {
        let addr = self
            .with_wallet(|wallet| {
                wallet
                    .get_internal_address(AddressIndex::New)
                    .expect("Couldn't get new address")
            })
            .await?;
        Ok(addr)
    }

    #[instrument(name = "keychain_wallet.sync", skip_all)]
    pub async fn sync<B: WalletSync + GetHeight + Send + Sync + 'static>(
        &self,
        blockchain: B,
    ) -> Result<(), BriaError> {
        self.with_wallet(move |wallet| wallet.sync(&blockchain, Default::default()))
            .await??;
        Ok(())
    }

    #[instrument(name = "keychain_wallet.balance", skip_all)]
    pub async fn balance(&self) -> Result<bdk::Balance, BriaError> {
        let balance = self.with_wallet(|wallet| wallet.get_balance()).await??;
        Ok(balance)
    }

    async fn with_wallet<F, R>(&self, f: F) -> Result<R, tokio::task::JoinError>
    where
        F: 'static + Send + FnOnce(Wallet<SqlxWalletDb>) -> R,
        R: Send + 'static,
    {
        let descriptor = self.descriptor.clone();
        let pool = self.pool.clone();
        let keychain_id = self.keychain_id;
        let network = self.network;
        let res = tokio::task::spawn_blocking(move || {
            let wallet = Wallet::new(
                descriptor.to_external_descriptor().as_str(),
                Some(descriptor.to_internal_descriptor().as_str()),
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
