mod helpers;

use bdk::{bitcoin::Network, blockchain::ElectrumBlockchain, electrum_client::Client};
use uuid::Uuid;

use bria::{wallet::*, xpub::*};

#[tokio::test]
async fn sync_wallet() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let keychain_id = Uuid::new_v4();
    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84'/0'/0'"))).unwrap();
    let keychain_cfg = WpkhKeyChainConfig::new(xpub);
    let wallet = KeychainWallet::new(pool, Network::Regtest, keychain_id.into(), keychain_cfg);

    let addr = wallet.new_external_address().await?;
    assert_eq!(
        addr.to_string(),
        "bcrt1qzg4a08kc2xrp08d9k5jadm78ehf7catp735zn0"
    );

    let bitcoind = helpers::bitcoind_client()?;
    helpers::fund_addr(&bitcoind, &addr, 1)?;
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let electrum_host = std::env::var("ELECTRUM_HOST").unwrap_or("localhost".to_string());
    let electrum_url = format!("{electrum_host}:50001");
    let blockchain = ElectrumBlockchain::from(Client::new(&electrum_url)?);
    wallet.sync(blockchain).await?;
    assert_eq!(wallet.balance().await?.untrusted_pending, 100_000_000);
    Ok(())
}
