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
    keychain_id: KeychainId,
    descriptor: String,
}

impl BdkWallet {
    pub fn new(pool: PgPool, keychain_id: KeychainId, descriptor: String) -> Self {
        Self {
            pool,
            keychain_id,
            descriptor,
        }
    }

    pub async fn next_address(&self) -> Result<bdk::wallet::AddressInfo, tokio::task::JoinError> {
        let descriptor = self.descriptor.clone();
        let pool = self.pool.clone();
        let keychain_id = self.keychain_id.clone();
        let address = tokio::task::spawn_blocking(move || {
            let wallet = Wallet::new(
                descriptor.as_str(),
                None,
                Network::Testnet,
                SqlxWalletDb::new(pool, keychain_id),
            )
            .expect("Couldn't construct wallet");
            wallet
                .get_address(AddressIndex::New)
                .expect("Couldn't get new address")
        })
        .await?;
        Ok(address)
    }
}

#[cfg(test)]
mod tests {
    use crate::bdk::pg::*;
    use bdk::{wallet::AddressIndex, Wallet};
    use bitcoin::Network;

    #[tokio::test]
    async fn test() {
        // let xpub = "tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4";
        // let descriptor = format!("wpkh({})", xpub);

        // let address = tokio::task::spawn_blocking(move || {
        //     let wallet = Wallet::new(&descriptor, None, Network::Testnet, SqlxWalletDb::new())
        //         .expect("Couldn't construct wallet");
        //     wallet
        //         .get_address(AddressIndex::New)
        //         .expect("Couldn't get new address")
        // })
        // .await;
        // println!("{:?}", address);
        // assert_eq!(1, 1);
    }
}
