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

    let domain_current_keychain_id = Uuid::new_v4();
    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84'/0'/0'"))).unwrap();
    let keychain_cfg = WpkhKeyChainConfig::new(xpub);
    let domain_current_keychain = KeychainWallet::new(
        pool.clone(),
        Network::Regtest,
        domain_current_keychain_id.into(),
        keychain_cfg,
    );

    let other_wallet_current_keychain = helpers::random_bdk_wallet()?;
    let other_wallet_deprecated_keychain = helpers::random_bdk_wallet()?;

    let domain_addr = domain_current_keychain.new_external_address().await?;
    let other_current_addr = other_wallet_current_keychain.get_address(AddressIndex::New)?;
    let other_deprecated_addr = other_wallet_deprecated_keychain.get_address(AddressIndex::New)?;

    let bitcoind = helpers::bitcoind_client()?;
    helpers::fund_addr(&bitcoind, &domain_addr, 5)?;
    helpers::fund_addr(&bitcoind, &other_current_addr, 5)?;
    helpers::fund_addr(&bitcoind, &other_deprecated_addr, 2)?;
    helpers::gen_blocks(&bitcoind, 6)?;

    let blockchain = helpers::electrum_blockchain()?;
    for _ in 0..5 {
        other_wallet_current_keychain.sync(&blockchain, Default::default())?;
        if other_wallet_current_keychain.get_balance()?.get_spendable() > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    other_wallet_deprecated_keychain.sync(&blockchain, Default::default())?;
    assert!(
        other_wallet_deprecated_keychain
            .get_balance()?
            .get_spendable()
            > 150_000_000
    );
    domain_current_keychain.sync(blockchain).await?;

    let fee = FeeRate::from_sat_per_vb(1.0);
    let builder = PsbtBuilder::new()
        .consolidate_deprecated_keychains(true)
        .fee_rate(fee)
        .accept_wallets();

    let domain_wallet_id = WalletId::new();
    let other_wallet_id = WalletId::new();
    let send_amount = 300_000_000;
    let payouts_one = vec![Payout {
        id: PayoutId::new(),
        wallet_id: domain_wallet_id,
        destination: PayoutDestination::OnchainAddress {
            value: "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap(),
        },
        satoshis: send_amount,
    }];
    let payouts_two = vec![Payout {
        id: PayoutId::new(),
        wallet_id: other_wallet_id,
        destination: PayoutDestination::OnchainAddress {
            value: "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap(),
        },
        satoshis: 300_000_000,
    }];
    let builder = builder
        .wallet_payouts(domain_wallet_id, payouts_one)
        .accept_current_keychain();
    let builder = domain_current_keychain
        .dispatch_bdk_wallet(builder)
        .await?
        .next_wallet();
    let other_wallet_current_keychain_id =
        uuid::uuid!("00000000-0000-0000-0000-000000000001").into();
    let other_wallet_deprecated_keychain_id =
        uuid::uuid!("00000000-0000-0000-0000-000000000002").into();
    let builder = builder
        .wallet_payouts(other_wallet_id, payouts_two)
        .visit_bdk_wallet(
            other_wallet_deprecated_keychain_id,
            &other_wallet_deprecated_keychain,
        )?
        .accept_current_keychain()
        .visit_bdk_wallet(
            other_wallet_current_keychain_id,
            &other_wallet_current_keychain,
        )?;
    let FinishedPsbtBuild {
        psbt: unsigned_psbt,
        ..
    } = builder.finish()?;

    let mut unsigned_psbt = unsigned_psbt.expect("unsigned psbt");
    assert_eq!(unsigned_psbt.inputs.len(), 3);
    assert_eq!(unsigned_psbt.outputs.len(), 4);

    other_wallet_current_keychain.sign(&mut unsigned_psbt, SignOptions::default())?;
    other_wallet_deprecated_keychain.sign(&mut unsigned_psbt, SignOptions::default())?;
    let mut lnd_client = helpers::lnd_signing_client().await?;
    let signed_psbt = lnd_client.sign_psbt(&unsigned_psbt).await?;
    let tx = domain_current_keychain
        .finalize_psbt(signed_psbt)
        .await?
        .extract_tx();
    helpers::electrum_blockchain()?.broadcast(&tx)?;

    Ok(())
}
