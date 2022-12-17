mod helpers;

use bdk::bitcoin::Network;
use uuid::Uuid;

use bria::{wallet::*, xpub::*};

#[tokio::test]
async fn new_address() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let keychain_id = Uuid::new_v4();
    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84'/0'/0'"))).unwrap();
    let keychain_cfg = WpkhKeyChainConfig::new(xpub);
    let wallet = KeychainWallet::new(
        pool.clone(),
        Network::Regtest,
        keychain_id.into(),
        keychain_cfg,
    );

    let addr = wallet.new_external_address().await?;
    assert_eq!(
        addr.to_string(),
        "bcrt1qzg4a08kc2xrp08d9k5jadm78ehf7catp735zn0"
    );

    let keychain_id = Uuid::new_v4();
    let xpub = XPub::try_from(("tpubDD6sGNgWVAeKaMGF5XkfBhMAuSqjoiqUoSM7Dmf11auxu41PDg1AL4LDwTkuVEMUS2zY51zPESy1xr26cLj7BZHfwZQHd4Xf1Ym5WbvAMru", Some("m/86'/0'/0'"))).unwrap();
    let keychain_cfg = TrKeyChainConfig::new(xpub);
    let wallet = KeychainWallet::new(pool, Network::Regtest, keychain_id.into(), keychain_cfg);

    let addr = wallet.new_external_address().await?;
    assert_eq!(
        addr.to_string(),
        "bcrt1p7dr79qw9j5wrc5hyva5rzaqygcmzdp00msqh02l45szlx5rae38qjhmf4a"
    );

    Ok(())
}
