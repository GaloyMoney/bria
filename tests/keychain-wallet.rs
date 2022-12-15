mod helpers;

use bdk::{
    bitcoin::Network,
    blockchain::{Blockchain, ElectrumBlockchain},
    electrum_client::Client,
    FeeRate,
};
use uuid::Uuid;

use bria::{signing_client::*, wallet::*, xpub::*};

#[tokio::test]
async fn end_to_end() -> anyhow::Result<()> {
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

    let electrum_host = std::env::var("ELECTRUM_HOST").unwrap_or("localhost".to_string());
    let electrum_url = format!("{electrum_host}:50001");
    for _ in 0..5 {
        let blockchain = ElectrumBlockchain::from(Client::new(&electrum_url)?);
        wallet.sync(blockchain).await?;
        let balance = wallet.balance().await?;
        if balance.untrusted_pending == 100_000_00 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    assert_eq!(wallet.balance().await?.untrusted_pending, 100_000_000);

    helpers::gen_blocks(&bitcoind, 6)?;

    for _ in 0..10 {
        let blockchain = ElectrumBlockchain::from(Client::new(&electrum_url)?);
        wallet.sync(blockchain).await?;
        let balance = wallet.balance().await?;
        if balance.untrusted_pending == 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    let balance = wallet.balance().await?;
    assert_eq!(balance.untrusted_pending, 0);
    let previous_spendable = balance.get_spendable();
    let send_amount = 100_000;

    let fee = FeeRate::from_sat_per_vb(1.0);
    let destinations = vec![(
        "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap(),
        send_amount,
    )];
    let res = wallet.prep_psbt(destinations, fee).await;
    assert!(res.is_ok());
    let unsigned_psbt = res.unwrap().unwrap().0;

    let mut lnd_client = helpers::lnd_signing_client().await?;
    let signed_psbt = lnd_client.sign_psbt(&unsigned_psbt).await?;
    let tx = wallet.finalize_psbt(signed_psbt).await?.extract_tx();
    let electrum = ElectrumBlockchain::from(Client::new(&electrum_url)?);
    electrum.broadcast(&tx)?;
    helpers::gen_blocks(&bitcoind, 6)?;

    for _ in 0..10 {
        let blockchain = ElectrumBlockchain::from(Client::new(&electrum_url)?);
        wallet.sync(blockchain).await?;
        let balance = wallet.balance().await?;
        if balance.get_spendable() < previous_spendable - send_amount {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    let balance = wallet.balance().await?;
    assert!(balance.get_spendable() < previous_spendable - send_amount);
    assert!(balance.get_spendable() > previous_spendable - send_amount - 1000);

    Ok(())
}
