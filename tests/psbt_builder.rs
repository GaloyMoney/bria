mod helpers;

use bdk::{bitcoin::Network, blockchain::Blockchain, wallet::AddressIndex, FeeRate, SignOptions};
use serial_test::serial;
use uuid::Uuid;

use bria::{primitives::*, wallet::*, xpub::*};

#[tokio::test]
#[serial]
async fn build_psbt() -> anyhow::Result<()> {
    // ChatGPT description of this test:
    //
    // This test is designed to cover specific scenarios to ensure the proper functioning of the PSBT
    // building process in a multi-wallet and multi-keychain environment. Two notable properties are
    // included as part of the test:
    //
    // 1. A wallet that spends without a change output: The test sets up a domain wallet that receives
    //    funding and sends a transaction without generating a change output. This scenario is created
    //    by carefully selecting the amount to be sent, such that it equals the funded amount minus a
    //    predefined fee. The test verifies that there is no change output for this wallet by asserting
    //    that the change satoshis are zero and that there is no change outpoint present.
    //
    // 2. A wallet with 2 keychains that consolidates UTXOs from the deprecated keychain: The test
    //    creates another wallet with two keychains - one current and one deprecated. Both keychains
    //    receive funding, simulating a real-world scenario where a wallet has UTXOs in a deprecated
    //    keychain. The `PsbtBuilder` is configured to consolidate deprecated keychains using the
    //    `consolidate_deprecated_keychains(true)` method. The test verifies that the UTXOs from both
    //    the current and deprecated keychains are included in the PSBT, ensuring that the consolidation
    //    of UTXOs from the deprecated keychain takes place as expected.
    //
    // By including these properties in the test, it ensures that the PSBT building process can handle
    // scenarios where a wallet spends without a change output and properly consolidates UTXOs from
    // deprecated keychains.

    let pool = helpers::init_pool().await?;

    let domain_current_keychain_id = Uuid::new_v4();
    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84'/0'/0'"))).unwrap();
    let keychain_cfg = KeychainConfig::wpkh(xpub);
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
    let other_change_address =
        other_wallet_current_keychain.get_internal_address(AddressIndex::New)?;
    let other_deprecated_addr = other_wallet_deprecated_keychain.get_address(AddressIndex::New)?;

    let bitcoind = helpers::bitcoind_client()?;
    let wallet_funding = 7;
    let wallet_funding_sats = Satoshis::from_btc(rust_decimal::Decimal::from(wallet_funding));
    helpers::fund_addr(&bitcoind, &domain_addr, wallet_funding)?;
    helpers::fund_addr(&bitcoind, &other_current_addr, wallet_funding - 2)?;
    helpers::fund_addr(&bitcoind, &other_deprecated_addr, 2)?;
    helpers::gen_blocks(&bitcoind, 10)?;

    let blockchain = helpers::electrum_blockchain().await?;
    for _ in 0..5 {
        other_wallet_current_keychain.sync(&blockchain, Default::default())?;
        other_wallet_deprecated_keychain.sync(&blockchain, Default::default())?;
        if other_wallet_current_keychain.get_balance()?.get_spendable() > 0
            && other_wallet_deprecated_keychain
                .get_balance()?
                .get_spendable()
                > 0
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    domain_current_keychain.sync(blockchain).await?;

    let fee = FeeRate::from_sat_per_vb(1.0);
    let builder = PsbtBuilder::new()
        .consolidate_deprecated_keychains(true)
        .fee_rate(fee)
        .accept_wallets();

    let domain_wallet_id = WalletId::new();
    let domain_send_amount = wallet_funding_sats - Satoshis::from(155);
    let other_wallet_id = WalletId::new();
    let send_amount = Satoshis::from(100_000_000);
    let destination: bitcoin::Address = "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap();
    let payouts_one = vec![(Uuid::new_v4(), destination.clone(), domain_send_amount)];

    let payouts_two = vec![
        (Uuid::new_v4(), destination.clone(), send_amount),
        (Uuid::new_v4(), destination.clone(), send_amount),
        (Uuid::new_v4(), destination, send_amount * 10),
    ];

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
        included_payouts,
        included_utxos,
        wallet_totals,
        fee_satoshis,
        ..
    } = builder.finish();
    assert_eq!(
        included_payouts
            .get(&domain_wallet_id)
            .expect("wallet not included in payouts")
            .len(),
        1
    );
    assert_eq!(
        included_payouts
            .get(&other_wallet_id)
            .expect("wallet not included in payouts")
            .len(),
        2
    );
    assert_eq!(
        included_utxos
            .get(&other_wallet_id)
            .expect("wallet not included in utxos")
            .get(&other_wallet_deprecated_keychain_id)
            .expect("keychain not included in utxos")
            .len(),
        1
    );
    assert_eq!(
        included_utxos
            .get(&other_wallet_id)
            .expect("wallet not included in utxos")
            .get(&other_wallet_current_keychain_id)
            .expect("keychain not included in utxos")
            .len(),
        1
    );
    assert_eq!(wallet_totals.len(), 2);
    let domain_wallet_total = wallet_totals.get(&domain_wallet_id).unwrap();
    assert_eq!(domain_wallet_total.input_satoshis, wallet_funding_sats);
    assert_eq!(domain_wallet_total.output_satoshis, domain_send_amount);
    assert_eq!(domain_wallet_total.change_satoshis, Satoshis::ZERO);
    assert!(domain_wallet_total.change_outpoint.is_none());
    assert_eq!(
        domain_wallet_total.output_satoshis + domain_wallet_total.fee_satoshis,
        domain_wallet_total.input_satoshis
    );

    let other_wallet_total = wallet_totals.get(&other_wallet_id).unwrap();
    assert_eq!(other_wallet_total.input_satoshis, wallet_funding_sats);
    assert_eq!(other_wallet_total.change_address, other_change_address);
    assert_eq!(other_wallet_total.fee_satoshis, Satoshis::from(193));
    assert_eq!(
        other_wallet_total
            .change_outpoint
            .expect("no change output")
            .vout,
        2
    );

    let mut unsigned_psbt = unsigned_psbt.expect("unsigned psbt");
    let total_tx_outs = unsigned_psbt
        .unsigned_tx
        .output
        .iter()
        .fold(0, |acc, out| acc + out.value);
    let total_summary_outs = wallet_totals
        .values()
        .fold(Satoshis::from(0), |acc, total| {
            acc + total.output_satoshis + total.change_satoshis
        });
    assert_eq!(total_tx_outs, u64::from(total_summary_outs));
    assert_eq!(total_tx_outs, u64::from(total_summary_outs));
    let total_summary_fees = wallet_totals
        .values()
        .fold(Satoshis::from(0), |acc, total| acc + total.fee_satoshis);
    assert_eq!(total_summary_fees, fee_satoshis);
    assert!(unsigned_psbt.inputs.len() >= 3);
    assert_eq!(unsigned_psbt.outputs.len(), 4);

    other_wallet_current_keychain.sign(&mut unsigned_psbt, SignOptions::default())?;
    other_wallet_deprecated_keychain.sign(&mut unsigned_psbt, SignOptions::default())?;
    let mut lnd_client = helpers::lnd_signing_client().await?;
    let signed_psbt = lnd_client.sign_psbt(&unsigned_psbt).await?;
    let tx = domain_current_keychain
        .finalize_psbt(signed_psbt)
        .await?
        .extract_tx();
    helpers::electrum_blockchain().await?.broadcast(&tx)?;

    Ok(())
}
