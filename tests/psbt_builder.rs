mod helpers;

use rand::Rng;
use serial_test::serial;

use bdk::{bitcoin::Network, blockchain::Blockchain, wallet::AddressIndex, FeeRate, SignOptions};
use uuid::Uuid;

use bria::{primitives::*, utxo::*, wallet::*, xpub::*};

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

    let external = "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/0/*)#l6n08zmr";
    let internal = "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/1/*)#wwkw6htm";
    let domain_current_keychain_id = Uuid::new_v4();
    let keychain_cfg = KeychainConfig::try_from((external.as_ref(), internal.as_ref()))?;
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

    let bitcoind = helpers::bitcoind_client().await?;
    let wallet_funding = 700_000_000;
    let wallet_funding_sats = Satoshis::from(wallet_funding);
    let tx_id = helpers::fund_addr(&bitcoind, &domain_addr, wallet_funding)?;
    helpers::fund_addr(&bitcoind, &other_current_addr, wallet_funding - 200_000_000)?;
    helpers::fund_addr(&bitcoind, &other_deprecated_addr, 200_000_000)?;
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
    while !find_tx_id(&pool, domain_current_keychain_id, tx_id).await? {
        let blockchain = helpers::electrum_blockchain().await?;
        domain_current_keychain.sync(blockchain).await?;
    }

    let fee = FeeRate::from_sat_per_vb(1.0);
    let cfg = PsbtBuilderConfig::builder()
        .consolidate_deprecated_keychains(true)
        .fee_rate(fee)
        .build()
        .unwrap();
    let builder = PsbtBuilder::new(cfg);

    let domain_wallet_id = WalletId::new();
    let domain_send_amount = wallet_funding_sats - Satoshis::from(155);
    let other_wallet_id = WalletId::new();
    let send_amount = Satoshis::from(100_000_000);
    let destination = Address::parse_from_trusted_source("mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU");
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
        domain_wallet_total.output_satoshis + domain_wallet_total.total_fee_satoshis,
        domain_wallet_total.input_satoshis
    );
    let other_wallet_total = wallet_totals.get(&other_wallet_id).unwrap();
    assert_eq!(other_wallet_total.input_satoshis, wallet_funding_sats);
    assert_eq!(other_wallet_total.change_address, other_change_address);
    assert_eq!(other_wallet_total.total_fee_satoshis, Satoshis::from(193));
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
        .fold(Satoshis::from(0), |acc, total| {
            acc + total.total_fee_satoshis
        });
    assert_eq!(total_summary_fees, fee_satoshis);
    assert!(unsigned_psbt.inputs.len() >= 3);
    assert_eq!(unsigned_psbt.outputs.len(), 4);

    other_wallet_current_keychain.sign(&mut unsigned_psbt, SignOptions::default())?;
    other_wallet_deprecated_keychain.sign(&mut unsigned_psbt, SignOptions::default())?;
    let mut bitcoind_client = helpers::bitcoind_signing_client().await?;
    let signed_psbt = bitcoind_client.sign_psbt(&unsigned_psbt).await?;
    let tx = domain_current_keychain
        .finalize_psbt(signed_psbt)
        .await?
        .expect("Finalize should have completed")
        .extract_tx();
    helpers::electrum_blockchain().await?.broadcast(&tx)?;

    Ok(())
}

#[tokio::test]
#[serial]
async fn build_psbt_with_cpfp() -> anyhow::Result<()> {
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
    let domain_addr = domain_current_keychain.new_external_address().await?;

    let bitcoind = helpers::bitcoind_client().await?;
    let wallet_funding = 500_000_000;
    let wallet_funding_sats = Satoshis::from(wallet_funding);
    helpers::fund_addr(&bitcoind, &domain_addr, wallet_funding)?;
    helpers::gen_blocks(&bitcoind, 10)?;
    let fee_bump_funding: u64 = rand::thread_rng().gen_range(100_000_000..=201_000_000);
    let fee_bump_funding_sats = Satoshis::from(fee_bump_funding);
    let domain_addr = domain_current_keychain.new_external_address().await?;
    let tx_id = helpers::fund_addr(&bitcoind, &domain_addr, fee_bump_funding)?;
    let (outpoint, cpfp_tx_fee, cpfp_tx_vsize) =
        helpers::lookup_tx_info(&bitcoind, tx_id, u64::from(fee_bump_funding_sats))?;

    let attributions = std::iter::once((
        tx_id,
        FeeWeightAttribution {
            batch_id: Some(BatchId::new()),
            tx_id,
            fee: cpfp_tx_fee,
            vbytes: cpfp_tx_vsize,
        },
    ))
    .collect();
    let cpfp_utxos = vec![CpfpUtxo {
        keychain_id: domain_current_keychain_id.into(),
        outpoint,
        value: fee_bump_funding_sats,
        attributions,
    }];
    let sats_per_vbyte: f64 = 100.0;
    let fee = FeeRate::from_sat_per_vb(sats_per_vbyte as f32);
    let cfg = PsbtBuilderConfig::builder()
        .consolidate_deprecated_keychains(true)
        .fee_rate(fee)
        .cpfp_utxos(
            vec![(KeychainId::from(domain_current_keychain_id), cpfp_utxos)]
                .into_iter()
                .collect(),
        )
        .build()
        .unwrap();
    let builder = PsbtBuilder::new(cfg);

    let domain_wallet_id = WalletId::new();
    let domain_send_amount = wallet_funding_sats - Satoshis::from(100_000_000);
    let destination = Address::parse_from_trusted_source("mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU");
    let payouts_one = vec![(Uuid::new_v4(), destination.clone(), domain_send_amount)];

    let builder = builder
        .wallet_payouts(domain_wallet_id, payouts_one)
        .accept_current_keychain();
    while !find_tx_id(&pool, domain_current_keychain_id, tx_id).await? {
        let blockchain = helpers::electrum_blockchain().await?;
        domain_current_keychain.sync(blockchain).await?;
    }
    let builder = domain_current_keychain
        .dispatch_bdk_wallet(builder)
        .await?
        .next_wallet();
    let FinishedPsbtBuild {
        psbt: unsigned_psbt,
        included_payouts,
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
    assert_eq!(wallet_totals.len(), 1);
    let domain_wallet_total = wallet_totals.get(&domain_wallet_id).unwrap();
    assert_eq!(domain_wallet_total.output_satoshis, domain_send_amount);
    assert_eq!(
        domain_wallet_total.output_satoshis
            + domain_wallet_total.total_fee_satoshis
            + domain_wallet_total.change_satoshis,
        domain_wallet_total.input_satoshis
    );
    let cpfp_allocations = &domain_wallet_total.cpfp_allocations;
    assert_eq!(cpfp_allocations.len(), 1);

    let unsigned_psbt = unsigned_psbt.expect("unsigned psbt");
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
        .fold(Satoshis::from(0), |acc, total| {
            acc + total.total_fee_satoshis
        });
    assert_eq!(total_summary_fees, fee_satoshis);
    assert!(unsigned_psbt.inputs.len() >= 2);
    assert_eq!(unsigned_psbt.outputs.len(), 2);

    let mut lnd_client = helpers::lnd_signing_client().await?;
    let signed_psbt = lnd_client.sign_psbt(&unsigned_psbt).await?;
    let tx = domain_current_keychain
        .finalize_psbt(signed_psbt)
        .await?
        .expect("Finalize should have completed")
        .extract_tx();

    let size = tx.vsize();
    let actual_sats_per_vbyte = u64::from(fee_satoshis) as f64 / size as f64;
    assert!(actual_sats_per_vbyte > sats_per_vbyte);

    let combined_rate =
        u64::from(cpfp_tx_fee + fee_satoshis) as f64 / (size as f64 + cpfp_tx_vsize as f64);
    assert!(combined_rate >= sats_per_vbyte);
    assert!(combined_rate < sats_per_vbyte * 1.02);

    let sats_per_vbyte_without_cpfp_fees =
        (u64::from(fee_satoshis - domain_wallet_total.cpfp_fee_satoshis) as f64) / size as f64;
    assert!(sats_per_vbyte_without_cpfp_fees >= sats_per_vbyte);
    assert!(sats_per_vbyte_without_cpfp_fees < sats_per_vbyte * 1.02);

    helpers::electrum_blockchain().await?.broadcast(&tx)?;
    Ok(())
}

#[tokio::test]
#[serial]
async fn build_psbt_with_min_change_output() -> anyhow::Result<()> {
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
    let domain_addr = domain_current_keychain.new_external_address().await?;

    let bitcoind = helpers::bitcoind_client().await?;
    let wallet_funding = 100_000_000;
    let wallet_funding_sats = Satoshis::from(wallet_funding);
    helpers::fund_addr(&bitcoind, &domain_addr, wallet_funding)?;
    let tx_id = helpers::fund_addr(&bitcoind, &domain_addr, wallet_funding)?;
    helpers::gen_blocks(&bitcoind, 10)?;

    let sats_per_vbyte: f64 = 100.0;
    let fee = FeeRate::from_sat_per_vb(sats_per_vbyte as f32);
    let min_change = Satoshis::from(100_000_000);
    let cfg = PsbtBuilderConfig::builder()
        .consolidate_deprecated_keychains(true)
        .fee_rate(fee)
        .force_min_change_output(Some(min_change))
        .build()
        .unwrap();
    let builder = PsbtBuilder::new(cfg);

    let domain_wallet_id = WalletId::new();
    let domain_send_amount = wallet_funding_sats - Satoshis::from(50_000_000);
    let destination = Address::parse_from_trusted_source("mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU");
    let payouts_one = vec![(Uuid::new_v4(), destination.clone(), domain_send_amount)];

    let builder = builder
        .wallet_payouts(domain_wallet_id, payouts_one)
        .accept_current_keychain();
    while !find_tx_id(&pool, domain_current_keychain_id, tx_id).await? {
        let blockchain = helpers::electrum_blockchain().await?;
        domain_current_keychain.sync(blockchain).await?;
    }
    let builder = domain_current_keychain
        .dispatch_bdk_wallet(builder)
        .await?
        .next_wallet();
    let FinishedPsbtBuild { wallet_totals, .. } = builder.finish();
    assert_eq!(wallet_totals.len(), 1);
    let domain_wallet_total = wallet_totals.get(&domain_wallet_id).unwrap();
    assert!(domain_wallet_total.change_satoshis >= min_change);

    Ok(())
}

/// The Domain wallet receives a payjoin from another wallet
///
/// Test that the domain wallet is sending funds
/// and this gets merged with incoming payjoin in order to cut-through and create
/// only a single change output
///
/// depositor -> domain -> withdrawer
#[tokio::test]
#[serial]
async fn build_psbt_with_payjoin() -> anyhow::Result<()> {
    use payjoin::receive::v2::WantsOutputs;
    let pool = helpers::init_pool().await?;

    // set up the PsbtBuilder domain wallet
    let external = "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/0/*)#l6n08zmr";
    let internal = "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/1/*)#wwkw6htm";

    let domain_current_keychain_id = Uuid::new_v4();
    let keychain_cfg = KeychainConfig::try_from((external.as_ref(), internal.as_ref()))?;
    let domain_current_keychain = KeychainWallet::new(
        pool.clone(),
        Network::Regtest,
        domain_current_keychain_id.into(),
        keychain_cfg,
    );
    dbg!("new keychain");
    let domain_addr = domain_current_keychain.new_external_address().await?;
    let domain_change_address = domain_current_keychain.new_internal_address().await?;
    let bitcoind = helpers::bitcoind_client().await?;
    let domain_funding = 300_000_000;
    let domain_funding_sats = Satoshis::from(domain_funding);
    dbg!("funding");
    let tx_id = helpers::fund_addr(&bitcoind, &domain_addr, domain_funding)?;
    dbg!("funded");
    helpers::gen_blocks(&bitcoind, 10)?;
    while !find_tx_id(&pool, domain_current_keychain_id, tx_id).await? {
        let blockchain = helpers::electrum_blockchain().await?;
        domain_current_keychain.sync(blockchain).await?;
    }

    // Build WantsOutputs for payjoin
    // 1st build original_psbt for deposit depositor -> domain, not part of the builder (or a separate builder)
    let deposit_addr = domain_current_keychain.new_external_address().await?;
    let deposit_funding = 200_000_000;
    let deposit_funding_sats = Satoshis::from(deposit_funding);
    dbg!("creating funded psbt");
    let deposit_original_psbt =
        helpers::create_funded_psbt(&bitcoind, &deposit_addr.address, deposit_funding)?;
    use std::str::FromStr;
    let deposit_original_psbt =
        payjoin::bitcoin::Psbt::from_str(&deposit_original_psbt.to_string())?;
    let domain_owned_vout = deposit_original_psbt
        .unsigned_tx
        .output
        .iter()
        .position(|o| o.value.to_sat() == deposit_funding)
        .unwrap();
    let change_vout = if domain_owned_vout == 0 { 1 } else { 0 };
    let wants_outputs = WantsOutputs::for_psbt_mutation(
        deposit_original_psbt,
        change_vout,
        vec![domain_owned_vout],
        payjoin::bitcoin::Address::from_str(&deposit_addr.address.to_string())?.assume_checked(),
    );

    let fee = FeeRate::from_sat_per_vb(1.0);
    let cfg = PsbtBuilderConfig::builder()
        .consolidate_deprecated_keychains(false) // for simplicity, we don't consolidate here
        .fee_rate(fee)
        .wants_outputs(Some(wants_outputs))
        .build()
        .unwrap();
    let builder = PsbtBuilder::new(cfg);

    let domain_wallet_id = WalletId::new();
    let withdrawal_funding = 400_000_000;
    let withdrawal_funding_sats = Satoshis::from(withdrawal_funding);
    // Send funds from domain to an address associated with neither depositor nor withdrawer
    let withdrawer_destination =
        Address::parse_from_trusted_source("mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU");
    let payouts = vec![(
        Uuid::new_v4(),
        withdrawer_destination.clone(),
        withdrawal_funding_sats,
    )];
    let builder = builder
        .wallet_payouts(domain_wallet_id, payouts)
        .accept_current_keychain();

    let builder = domain_current_keychain
        .dispatch_bdk_wallet(builder)
        .await?
        .next_wallet();

    // First, propose an original_psbt
    let FinishedPsbtBuild {
        psbt: unsigned_psbt,
        included_payouts,
        included_utxos,
        wallet_totals,
        fee_satoshis,
        provisional_proposal,
        ..
    } = builder.finish();
    assert_eq!(
        included_payouts
            .get(&domain_wallet_id)
            .expect("wallet not included in payouts")
            .len(),
        1
    );
    assert_eq!(wallet_totals.len(), 1);
    // let other_wallet_total = wallet_totals.get(&depositor_wallet_id).unwrap();
    // assert!(other_wallet_total.change_outpoint.is_none());
    // assert_eq!(other_wallet_total.change_address, other_change_address);
    // assert_eq!(
    //     other_wallet_total.output_satoshis
    //         + other_wallet_total.change_satoshis
    //         + other_wallet_total.total_fee_satoshis,
    //     other_wallet_total.input_satoshis
    // );
    // assert_eq!(other_wallet_total.total_fee_satoshis, Satoshis::from(155));

    let mut unsigned_psbt = unsigned_psbt.expect("unsigned psbt");
    // let total_tx_outs = unsigned_psbt
    //     .unsigned_tx
    //     .output
    //     .iter()
    //     .fold(0, |acc, out| acc + out.value);
    // let total_summary_outs = wallet_totals
    //     .values()
    //     .fold(Satoshis::from(0), |acc, total| {
    //         acc + total.output_satoshis + total.change_satoshis
    //     });
    // assert_eq!(total_tx_outs, u64::from(total_summary_outs));
    // assert_eq!(total_tx_outs, u64::from(total_summary_outs));
    // let total_summary_fees = wallet_totals
    //     .values()
    //     .fold(Satoshis::from(0), |acc, total| {
    //         acc + total.total_fee_satoshis
    //     });
    // assert_eq!(total_summary_fees, fee_satoshis);
    assert!(unsigned_psbt.inputs.len() >= 1); // from payjoin sender only
    assert_eq!(unsigned_psbt.outputs.len(), 3); // withdrawal, sender change, domain change

    let mut bitcoind_client = helpers::bitcoind_signing_client().await?;
    dbg!("signing with bitcoind");
    let signed_psbt = bitcoind_client.sign_psbt(&unsigned_psbt).await?;
    dbg!(&signed_psbt.to_string());
    let _tx = domain_current_keychain
        .finalize_psbt(signed_psbt) // FIXME do we need to finalize before or after payjoin sender signs?
        .await?
        .expect("Finalize should have completed")
        .extract_tx();
    // The tx won't be able to be broadcast because it's missing signature data from the payjoin sender
    Ok(())
}

async fn find_tx_id(
    pool: &sqlx::PgPool,
    keychain_id: Uuid,
    tx_id: bitcoin::Txid,
) -> anyhow::Result<bool> {
    let utxos = sqlx::query!(
        r#"SELECT count(*) as "count!" FROM bdk_utxos WHERE keychain_id = $1 AND tx_id = $2"#,
        keychain_id,
        tx_id.to_string(),
    )
    .fetch_one(pool)
    .await?;
    Ok(utxos.count != 0)
}
