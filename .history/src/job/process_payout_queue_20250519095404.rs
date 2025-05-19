use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::instrument;

use super::error::JobError;
use crate::{
    batch::*, fees::FeesClient, payout::*, payout_queue::*, primitives::*, utxo::*, wallet::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessPayoutQueueData {
    pub(super) payout_queue_id: PayoutQueueId,
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
    #[serde(flatten)]
    pub(super) tracing_data: HashMap<String, String>,
}

#[instrument(
    name = "job.process_payout_queue",
    skip_all,
    fields(
        n_unbatched_payouts,
        payout_queue_name,
        n_reserved_utxos,
        n_cpfp_utxos,
        tx_id,
        total_fee_sats,
        cpfp_fee_sats,
        total_change_sats,
        psbt,
        batch_id,
        payout_queue_id
    ),
    err
)]
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) async fn execute<'a>(
    pool: sqlx::PgPool,
    payouts: Payouts,
    wallets: Wallets,
    payout_queues: PayoutQueues,
    batches: Batches,
    utxos: Utxos,
    data: ProcessPayoutQueueData,
    fees_client: FeesClient,
) -> Result<
    (
        ProcessPayoutQueueData,
        Option<(sqlx::Transaction<'a, sqlx::Postgres>, Vec<WalletId>)>,
    ),
    JobError,
> {
    let payout_queue = payout_queues
        .find_by_id(data.account_id, data.payout_queue_id)
        .await?;
    let mut tx = pool.begin().await?;
    let mut unbatched_payouts = payouts
        .list_unbatched(&mut tx, data.account_id, data.payout_queue_id)
        .await?;
    let fee_rate = fees_client
        .fee_rate(payout_queue.config.tx_priority)
        .await?;
    let FinishedPsbtBuild {
        psbt,
        included_payouts,
        included_utxos,
        wallet_totals,
        tx_id,
        fee_satoshis,
        ..
    } = construct_psbt(
        &pool,
        &mut tx,
        &unbatched_payouts,
        &utxos,
        &wallets,
        payout_queue,
        fee_rate,
        false,
    )
    .await?;

    let span = tracing::Span::current();
    if let (Some(tx_id), Some(psbt)) = (tx_id, psbt) {
        span.record("tx_id", tracing::field::display(tx_id));
        span.record("psbt", tracing::field::display(&psbt));

        let wallet_ids = wallet_totals.keys().copied().collect();
        span.record("batch_id", tracing::field::display(data.batch_id));
        span.record("total_fee_sats", tracing::field::display(fee_satoshis));
        span.record(
            "total_change_sats",
            tracing::field::display(
                wallet_totals
                    .values()
                    .fold(Satoshis::ZERO, |acc, v| acc + v.change_satoshis),
            ),
        );
        span.record(
            "cpfp_fee_sats",
            tracing::field::display(
                wallet_totals
                    .values()
                    .fold(Satoshis::ZERO, |acc, v| acc + v.cpfp_fee_satoshis),
            ),
        );
        let batch = NewBatch::builder()
            .account_id(data.account_id)
            .id(data.batch_id)
            .payout_queue_id(data.payout_queue_id)
            .tx_id(tx_id)
            .unsigned_psbt(psbt)
            .total_fee_sats(fee_satoshis)
            .wallet_summaries(
                wallet_totals
                    .into_iter()
                    .map(|(wallet_id, total)| (wallet_id, WalletSummary::from(total)))
                    .collect(),
            )
            .build()
            .expect("Couldn't build batch");

        // Not using a Box here causes an interesting compile error with rustc 1.69.0
        let included_utxos: Box<dyn Iterator<Item = (KeychainId, bitcoin::OutPoint)> + Send> =
            Box::new(included_utxos.into_iter().flat_map(|(_, keychain_map)| {
                keychain_map
                    .into_iter()
                    .flat_map(|(keychain_id, outpoints)| {
                        outpoints
                            .into_iter()
                            .map(move |outpoint| (keychain_id, outpoint))
                    })
            }));

        let batch_id = batch.id;
        batches.create_in_tx(&mut tx, batch).await?;
        utxos
            .reserve_utxos_in_batch(
                &mut tx,
                data.account_id,
                batch_id,
                data.payout_queue_id,
                fee_rate,
                included_utxos,
            )
            .await?;

        unbatched_payouts.commit_to_batch(
            tx_id,
            batch_id,
            included_payouts
                .into_values()
                .flat_map(|payouts| payouts.into_iter().map(|((id, _, _), vout)| (id, vout))),
        );

        if unbatched_payouts.n_not_batched() > 0 {
            queue_drain_error(unbatched_payouts.n_not_batched());
        }

        payouts.update_unbatched(&mut tx, unbatched_payouts).await?;

        Ok((data, Some((tx, wallet_ids))))
    } else {
        if unbatched_payouts.n_not_batched() > 0 {
            queue_drain_error(unbatched_payouts.n_not_batched());
        }
        Ok((data, None))
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn construct_psbt(
    pool: &sqlx::Pool<sqlx::Postgres>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    unbatched_payouts: &UnbatchedPayouts,
    utxos: &Utxos,
    wallets: &Wallets,
    payout_queue: PayoutQueue,
    fee_rate: bitcoin::FeeRate,
    for_estimation: bool,
) -> Result<FinishedPsbtBuild, JobError> {
    let span = tracing::Span::current();
    let PayoutQueue {
        id: queue_id,
        config: queue_cfg,
        name: queue_name,
        ..
    } = payout_queue;
    span.record("payout_queue_name", queue_name);
    span.record("payout_queue_id", tracing::field::display(queue_id));
    span.record("n_unbatched_payouts", unbatched_payouts.n_payouts());

    let wallets = wallets.find_by_ids(unbatched_payouts.wallet_ids()).await?;
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

    Ok(PsbtBuilder::construct_psbt(
        pool,
        cfg.for_estimation(for_estimation)
            .build()
            .expect("Couldn't build PsbtBuilderConfig"),
        tx_payouts,
        wallets,
    )
    .await?)
}

#[instrument(name = "job.queue_drain_error", fields(error = true, error.level, error.message))]
fn queue_drain_error(n_not_batched: usize) {
    let span = tracing::Span::current();
    span.record(
        "error.level",
        tracing::field::display(&tracing::Level::ERROR),
    );
    span.record("error.message", "Queue could not be drained");
}

impl From<WalletTotals> for WalletSummary {
    fn from(wt: WalletTotals) -> Self {
        let cpfp_details = wt
            .cpfp_allocations
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    v.into_iter()
                        .map(|(tx_id, (batch_id, bump_fee))| {
                            (
                                tx_id,
                                CpfpDetails {
                                    tx_id,
                                    batch_id,
                                    bump_fee,
                                },
                            )
                        })
                        .collect::<HashMap<bitcoin::Txid, CpfpDetails>>(),
                )
            })
            .collect();
        Self {
            wallet_id: wt.wallet_id,
            signing_keychains: wt.keychains_with_inputs,
            total_in_sats: wt.input_satoshis,
            total_spent_sats: wt.output_satoshis,
            total_fee_sats: wt.total_fee_satoshis,
            cpfp_fee_sats: wt.cpfp_fee_satoshis,
            cpfp_details,
            change_sats: wt.change_satoshis,
            change_address: wt
                .change_outpoint
                .map(|_| Address::from(wt.change_address.address)),
            change_outpoint: wt.change_outpoint,
            current_keychain_id: wt.change_keychain_id,
            batch_created_ledger_tx_id: None,
            batch_broadcast_ledger_tx_id: None,
        }
    }
}
