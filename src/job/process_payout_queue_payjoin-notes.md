```rs
        //-- THIS IS THE old process_payout_queue payjoin code
        //let wallet_id = unbatched_payouts.wallet_ids().into_iter()..first().unwrap(); // we know the length is one from the is_payjoin_eligible check
        // DEFINE OUTPUTS -------
        // ----------------------
        use rust_decimal::prelude::ToPrimitive;
        let replacement_outputs: Vec<payjoin::bitcoin::TxOut> = unbatched_payouts
            .into_iter()
            .flat_map(|(_wallet_id, payouts)|  payouts.into_iter())
            .map(|(_, address, sats)| {
                payjoin::bitcoin::TxOut {
                    value: payjoin::bitcoin::Amount::from_btc(sats.to_btc().to_f64().unwrap()).unwrap(),
                    script_pubkey: payjoin::bitcoin::ScriptBuf::from_bytes(address.script_pubkey().to_bytes()),
                }
            })
            .collect();

        // FIXME STUPID SIMPLIFICATION: pick first availabledrain address
        // FIXME bria can have multiple drain scripts since a queue 'receiver' is actually multiple wallets
        let drain_script = replacement_outputs.first().expect("no outputs to replace with").script_pubkey;
        let wants_inputs = wants_outputs.replace_receiver_outputs(replacement_outputs, &drain_script).unwrap().commit_outputs();

        // CONTRIBUTE INPUTS -------
        // -------------------------
        // payout queue config, batch signing job
        println!("contribute");
        // Don't throw an error. Continue optimistic process even if we can't contribute inputs.
        
        let available_wallets = wallets
            .list_by_account_id(data.account_id)
            .await
            .expect("Failed to list wallets");
        let keychain_ids = available_wallets
            .iter()
            .flat_map(|wallet| wallet.keychain_ids());
        let mut keychain_utxos = utxos.find_keychain_utxos(keychain_ids).await.expect("failed to find keychain utxos");
        let keychain_utxos = keychain_utxos
            .drain()
            .map(|(_, keychain_utxos)| keychain_utxos)
            .collect::<Vec<_>>();
        
        let mut available_inputs = keychain_utxos
            .iter()
            .flat_map(|keychain_utxos| keychain_utxos.utxos.iter());

        let candidate_inputs: HashMap<payjoin::bitcoin::Amount, payjoin::bitcoin::OutPoint> = available_inputs
            .clone()
            // Why is a utxo output value NOT saved in bitcoin::Amount? How can it be partial satoshis?
            .map(|i| {
                let txid = payjoin::bitcoin::Txid::from_str(&i.outpoint.txid.to_string()).unwrap();
                (
                    payjoin::bitcoin::Amount::from_sat(i.value.into()),
                    payjoin::bitcoin::OutPoint::new(txid, i.outpoint.vout),
                )
            })
            .collect();
        let selected_outpoint = wants_inputs
            .try_preserving_privacy(candidate_inputs)
            .expect("no privacy preserving utxo found");
        let selected_utxo = available_inputs
            .find(|i| {
                let txid = payjoin::bitcoin::Txid::from_str(&i.outpoint.txid.to_string()).unwrap();
                payjoin::bitcoin::OutPoint::new(txid, i.outpoint.vout) == selected_outpoint
            })
            .expect("This shouldn't happen. Failed to retrieve the privacy preserving utxo from those we provided to the seclector.");

        let txo_to_contribute = payjoin::bitcoin::TxOut {
            value: payjoin::bitcoin::Amount::from_sat(selected_utxo.value.into()),
            script_pubkey: payjoin::bitcoin::ScriptBuf::from_bytes(selected_utxo
                .address
                .clone()
                .expect("selected_utxo missing script")
                .script_pubkey().to_bytes()),
        };
        let provisional_proposal = wants_inputs.contribute_witness_inputs(vec![(selected_outpoint, txo_to_contribute)]).expect("failed to contribute inputs").commit_inputs();
        // --
```

```rs
use std::sync::{Arc, Mutex};
                    use std::sync::mpsc::{self, Sender, Receiver};
                    use std::thread;
                    use std::time::Duration;
                    use crate::payjoin::ProcessPsbtControl;

                    let (tx, rx): (Sender<ProcessPsbtControl>, Receiver<ProcessPsbtControl>) = mpsc::channel();
                    provisional_proposal.finalize_proposal(|psbt| {
                        let psbt = crate::payjoin::wallet_process_psbt(psbt.clone()).unwrap();
                        Ok(psbt.clone())
                    }, None, payjoin::bitcoin::FeeRate::from_sat_per_vb(100).unwrap());
                    // TODO
                    // TODO
                    // TODO


                    ```
                    ```rs

    #[instrument(name = "psbt_builder.construct_payjoin_psbt", skip_all)]
    pub async fn construct_payjoin_psbt(
        pool: &sqlx::PgPool,
        cfg: PsbtBuilderConfig,
        wants_outputs: payjoin::receive::v2::WantsOutputs,
        unbatched_payouts: HashMap<WalletId, Vec<TxPayout>>,
        mut wallets: HashMap<WalletId, WalletEntity>, // FIXME invariant where unbatched_payouts.wallet_ids().len() == 1  
    ) -> Result<FinishedPsbtBuild, BdkError> {
        let mut outer_builder: PsbtBuilder<AcceptingWalletState> = PsbtBuilder::new(cfg);

        let wallet_id = unbatched_payouts.keys().next().expect("unbatched_payouts must be non-empty");
        let payouts = unbatched_payouts.values().next().expect("unbatched_payouts must be non-empty");
        let wallet = wallets.remove(&wallet_id.clone()).expect("Wallet not found");

        let mut builder = outer_builder.wallet_payouts(*wallet_id, payouts.to_vec());
        for keychain in wallet.deprecated_keychain_wallets(pool.clone()) {
            builder = keychain.dispatch_bdk_wallet(builder).await?;
        }
        // include inputs and outputs:
        outer_builder = wallet
            .current_keychain_wallet(pool)
            .dispatch_bdk_wallet(builder.accept_current_keychain())
            .await?
            .next_wallet();

        //--
        //let wallet_id = unbatched_payouts.wallet_ids().into_iter()..first().unwrap(); // we know the length is one from the is_payjoin_eligible check
        // DEFINE OUTPUTS -------
        // ----------------------
        use rust_decimal::prelude::ToPrimitive;
        let replacement_outputs: Vec<payjoin::bitcoin::TxOut> = unbatched_payouts
            .into_iter()
            .flat_map(|(_wallet_id, payouts)|  payouts.into_iter())
            .map(|(_, address, sats)| {
                payjoin::bitcoin::TxOut {
                    value: payjoin::bitcoin::Amount::from_btc(sats.to_btc().to_f64().unwrap()).unwrap(),
                    script_pubkey: payjoin::bitcoin::ScriptBuf::from_bytes(address.script_pubkey().to_bytes()),
                }
            })
            .collect();

        // FIXME STUPID SIMPLIFICATION: pick first availabledrain address
        // FIXME bria can have multiple drain scripts since a queue 'receiver' is actually multiple wallets
        let drain_script = replacement_outputs.first().expect("no outputs to replace with").script_pubkey;
        let wants_inputs = wants_outputs.replace_receiver_outputs(replacement_outputs, &drain_script).unwrap().commit_outputs();

        // CONTRIBUTE INPUTS -------
        // -------------------------
        // payout queue config, batch signing job
        println!("contribute");
        // Don't throw an error. Continue optimistic process even if we can't contribute inputs.
        
        let available_wallets = wallets
            .list_by_account_id(data.account_id)
            .await
            .expect("Failed to list wallets");
        let keychain_ids = available_wallets
            .iter()
            .flat_map(|wallet| wallet.keychain_ids());
        let mut keychain_utxos = utxos.find_keychain_utxos(keychain_ids).await.expect("failed to find keychain utxos");
        let keychain_utxos = keychain_utxos
            .drain()
            .map(|(_, keychain_utxos)| keychain_utxos)
            .collect::<Vec<_>>();
        
        let mut available_inputs = keychain_utxos
            .iter()
            .flat_map(|keychain_utxos| keychain_utxos.utxos.iter());

        let candidate_inputs: HashMap<payjoin::bitcoin::Amount, payjoin::bitcoin::OutPoint> = available_inputs
            .clone()
            // Why is a utxo output value NOT saved in bitcoin::Amount? How can it be partial satoshis?
            .map(|i| {
                let txid = payjoin::bitcoin::Txid::from_str(&i.outpoint.txid.to_string()).unwrap();
                (
                    payjoin::bitcoin::Amount::from_sat(i.value.into()),
                    payjoin::bitcoin::OutPoint::new(txid, i.outpoint.vout),
                )
            })
            .collect();
        let selected_outpoint = wants_inputs
            .try_preserving_privacy(candidate_inputs)
            .expect("no privacy preserving utxo found");
        let selected_utxo = available_inputs
            .find(|i| {
                let txid = payjoin::bitcoin::Txid::from_str(&i.outpoint.txid.to_string()).unwrap();
                payjoin::bitcoin::OutPoint::new(txid, i.outpoint.vout) == selected_outpoint
            })
            .expect("This shouldn't happen. Failed to retrieve the privacy preserving utxo from those we provided to the seclector.");

        let txo_to_contribute = payjoin::bitcoin::TxOut {
            value: payjoin::bitcoin::Amount::from_sat(selected_utxo.value.into()),
            script_pubkey: payjoin::bitcoin::ScriptBuf::from_bytes(selected_utxo
                .address
                .clone()
                .expect("selected_utxo missing script")
                .script_pubkey().to_bytes()),
        };
        let provisional_proposal = wants_inputs.contribute_witness_inputs(vec![(selected_outpoint, txo_to_contribute)]).expect("failed to contribute inputs").commit_inputs();
        // --
        Ok(outer_builder.finish())
    }
```

```rs

#[allow(clippy::too_many_arguments)]
pub async fn construct_payjoin_psbt(
    pool: &sqlx::Pool<sqlx::Postgres>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    unbatched_payouts: &UnbatchedPayouts,
    utxos: &Utxos,
    wallets: &Wallets, // FIXME invariant where unbatched_payouts.wallet_ids().len() == 1
    payout_queue: PayoutQueue,
    fee_rate: bitcoin::FeeRate,
    for_estimation: bool,
    wants_outputs: WantsOutputs,
) -> Result<FinishedPsbtBuild, JobError> {
    let span = tracing::Span::current();
    let PayoutQueue {
        id: queue_id,
        config: queue_cfg,
        name: queue_name,
        ..
    } = payout_queue;
    span.record("payout_queue_name", queue_name);
    span.record("payout_queue_id", &tracing::field::display(queue_id));
    span.record("n_unbatched_payouts", unbatched_payouts.n_payouts());

    let wallets = wallets.find_by_ids(unbatched_payouts.wallet_ids()).await?;
    // inputs
    let reserved_utxos = {
        let keychain_ids = wallets.values().flat_map(|w| w.keychain_ids());
        utxos
            .outpoints_bdk_should_not_select(tx, keychain_ids)
            .await?
    };
    span.record(
        "n_reserved_utxos",
        reserved_utxos.values().fold(0, |acc, v| acc + v.len()),
    );

    span.record("n_cpfp_utxos", 0);

    let mut cfg = PsbtBuilderConfig::builder()
        .consolidate_deprecated_keychains(queue_cfg.consolidate_deprecated_keychains)
        .fee_rate(fee_rate)
        .reserved_utxos(reserved_utxos)
        .force_min_change_output(queue_cfg.force_min_change_sats);
    if !for_estimation && queue_cfg.should_cpfp() {
        let keychain_ids = wallets.values().flat_map(|w| w.keychain_ids());
        let utxos = utxos
            .find_cpfp_utxos(
                tx,
                keychain_ids,
                queue_id,
                queue_cfg.cpfp_payouts_detected_before(),
                queue_cfg
                    .cpfp_payouts_detected_before_block(crate::bdk::last_sync_time(pool).await?),
            )
            .await?;
        span.record(
            "n_cpfp_utxos",
            utxos.values().fold(0, |acc, v| acc + v.len()),
        );
        cfg = cfg.cpfp_utxos(utxos);
    }

    let tx_payouts = unbatched_payouts.into_tx_payouts();
    // TODO add proposal tx_payouts
    Ok(PsbtBuilder::construct_psbt(
        pool,
        cfg.for_estimation(for_estimation)
            .wants_outputs(Some(wants_outputs))
            .build()
            .expect("Couldn't build PsbtBuilderConfig"),
        tx_payouts,
        wallets,
    )
    .await?)
}
```