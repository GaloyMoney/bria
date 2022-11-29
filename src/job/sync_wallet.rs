use bdk::blockchain::ElectrumBlockchain;
use bitcoin::Network;
use electrum_client::Client;

use crate::{error::*, primitives::*, wallet::*};

pub async fn execute(
    pool: sqlx::PgPool,
    wallets: Wallets,
    network: Network,
    id: WalletId,
) -> Result<(), BriaError> {
    let keychain = wallets.find_by_id(id).await?;
    // let keychain_wallet = KeychainWallet::new(pool, network, id, keychain);
    // let electrum_url = "127.0.0.1:50001";
    // let blockchain = ElectrumBlockchain::from(Client::new(electrum_url).unwrap());
    // keychain_wallet.sync(blockchain).await?;
    Ok(())
}
