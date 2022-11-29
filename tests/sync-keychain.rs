mod helpers;

use bdk::bitcoin::Network;
use uuid::Uuid;

use bria::wallet::KeychainWallet;

#[tokio::test]
async fn sync_wallet() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let keychain_id = Uuid::new_v4();
    // let wallet = KeychainWallet::new(pool, Network::Regtest, keychain_id.into());

    Ok(())
}
