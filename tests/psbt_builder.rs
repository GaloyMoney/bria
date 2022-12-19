mod helpers;

use bdk::{
    bitcoin::Network,
    blockchain::{Blockchain, ElectrumBlockchain},
    electrum_client::Client,
    wallet::AddressIndex,
    FeeRate, SignOptions,
};
use uuid::Uuid;

use bria::{payout::*, primitives::*, signer::*, wallet::*, xpub::*};

#[tokio::test]
#[serial_test::serial]
async fn build_psbt() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let keychain_id = Uuid::new_v4();
    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84'/0'/0'"))).unwrap();
    let keychain_cfg = WpkhKeyChainConfig::new(xpub);
    let wallet_one = KeychainWallet::new(
        pool.clone(),
        Network::Regtest,
        keychain_id.into(),
        keychain_cfg,
    );
    let addr = wallet_one.new_external_address().await?;
    assert_eq!(
        addr.to_string(),
        "bcrt1qzg4a08kc2xrp08d9k5jadm78ehf7catp735zn0"
    );

    let bitcoind = helpers::bitcoind_client()?;
    let other_wallet = helpers::random_bdk_wallet()?;
    let other_addr = other_wallet.get_address(AddressIndex::New)?;
    helpers::fund_addr(&bitcoind, &addr, 1)?;
    helpers::fund_addr(&bitcoind, &other_addr, 2)?;
    helpers::gen_blocks(&bitcoind, 6)?;

    let blockchain = helpers::electrum_blockchain()?;
    for _ in 0..5 {
        other_wallet.sync(&blockchain, Default::default())?;
        if other_wallet.get_balance()?.get_spendable() > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    wallet_one.sync(blockchain).await?;
    let previous_spendable_one = wallet_one.balance().await?.get_spendable();
    let other_previous_spendable = other_wallet.get_balance()?.get_spendable();
    assert!(previous_spendable_one > 0);
    assert!(other_previous_spendable > 0);

    let fee = FeeRate::from_sat_per_vb(1.0);
    let builder = PsbtBuilder::new()
        .consolidate_deprecated_keychains(false)
        .fee_rate(fee)
        .begin_wallets();

    let first_wallet_id = WalletId::new();
    let second_wallet_id = WalletId::new();
    let send_amount = 10_000;
    let payouts_one = vec![Payout {
        id: PayoutId::new(),
        wallet_id: first_wallet_id,
        destination: PayoutDestination::OnchainAddress {
            value: "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap(),
        },
        satoshis: send_amount,
    }];
    let payouts_two = vec![Payout {
        id: PayoutId::new(),
        wallet_id: second_wallet_id,
        destination: PayoutDestination::OnchainAddress {
            value: "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap(),
        },
        satoshis: send_amount,
    }];
    let builder = builder.wallet_payouts(first_wallet_id, payouts_one);
    let builder = wallet_one.dispatch_bdk_wallet(builder).await?.next_wallet();
    let builder = builder.wallet_payouts(second_wallet_id, payouts_two);
    let other_keychain_id = KeychainId::new();
    let builder = builder.visit_bdk_wallet(other_keychain_id, &other_wallet)?;
    let FinishedPsbtBuild {
        psbt: unsigned_psbt,
    } = builder.finish()?;

    let mut unsigned_psbt = unsigned_psbt.expect("unsigned psbt");
    assert_eq!(unsigned_psbt.inputs.len(), 2);
    other_wallet.sign(&mut unsigned_psbt, SignOptions::default())?;
    let mut lnd_client = helpers::lnd_signing_client().await?;
    let signed_psbt = lnd_client.sign_psbt(&unsigned_psbt).await?;
    let tx = wallet_one.finalize_psbt(signed_psbt).await?.extract_tx();
    helpers::electrum_blockchain()?.broadcast(&tx)?;
    helpers::gen_blocks(&bitcoind, 6)?;
    let blockchain = helpers::electrum_blockchain()?;
    for _ in 0..10 {
        other_wallet.sync(&blockchain, Default::default())?;
        let balance = other_wallet.get_balance()?;
        if balance.get_spendable() < other_previous_spendable - send_amount {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    wallet_one.sync(blockchain).await?;
    let balance = wallet_one.balance().await?;
    assert!(balance.get_spendable() < previous_spendable_one - send_amount);
    let balance = other_wallet.get_balance()?;
    assert!(balance.get_spendable() < other_previous_spendable - send_amount);

    Ok(())
}
