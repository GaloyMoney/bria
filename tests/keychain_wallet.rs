mod helpers;

use bdk::bitcoin::Network;

use bria::{primitives::*, wallet::*};

#[tokio::test]
async fn new_address() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let keychain_id = KeychainId::new();
    let external = "wpkh([492ef832/84'/0'/0']tpubDCyf42ghP6mBAETwkz8AbXo97822jdUgHNug91bbX8GXpTP6ee298yGeiM5SvgL8Z85bcFyKioyRQokNh6J4eT3Fy8mgKDAKfynovRu3WzE/0/*)#gf2aklqx";
    let internal = "wpkh([492ef832/84'/0'/0']tpubDCyf42ghP6mBAETwkz8AbXo97822jdUgHNug91bbX8GXpTP6ee298yGeiM5SvgL8Z85bcFyKioyRQokNh6J4eT3Fy8mgKDAKfynovRu3WzE/1/*)#ea0ut2s7";
    let keychain_cfg = WalletKeychainDescriptors::try_from((external, internal)).unwrap();
    let wallet = KeychainWallet::new(pool.clone(), Network::Regtest, keychain_id, keychain_cfg);

    let addr = wallet.new_external_address().await?;
    assert_eq!(
        addr.to_string(),
        "bcrt1qcv9xq3me73wsv4scy6qvx3f24e3dnt56h9m9z6"
    );
    let addr = wallet
        .find_address_from_path(101, bdk::KeychainKind::External)
        .await?;
    assert_eq!(
        addr.to_string(),
        "bcrt1ql30ktsmtdfj6a7243xhfn8n35hyghyw2yj9alf"
    );

    Ok(())
}
