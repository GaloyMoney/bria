use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::instrument;

use crate::{batch::*, error::*, payout::*, payout_queue::*, primitives::*, utxo::*, wallet::*};

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
        txid,
        psbt,
        batch_id,
        payout_queue_id
    ),
    err
)]
#[allow(clippy::type_complexity)]
pub async fn execute<'a>(
    pool: sqlx::PgPool,
    payouts: Payouts,
    wallets: Wallets,
    payout_queues: PayoutQueues,
    batches: Batches,
    utxos: Utxos,
    data: ProcessPayoutQueueData,
) -> Result<
    (
        ProcessPayoutQueueData,
        Option<(sqlx::Transaction<'a, sqlx::Postgres>, Vec<WalletId>)>,
    ),
    BriaError,
> {
    let payout_queue = payout_queues
        .find_by_id(data.account_id, data.payout_queue_id)
        .await?;
    let mut unbatched_payouts = payouts
        .list_unbatched(data.account_id, data.payout_queue_id)
        .await?;
    let mut tx = pool.begin().await?;

    let FinishedPsbtBuild {
        psbt,
        included_payouts,
        included_utxos,
        wallet_totals,
        tx_id,
        fee_satoshis,
        ..
    } = construct_psbt(
        pool,
        &mut tx,
        &unbatched_payouts,
        &utxos,
        wallets,
        payout_queue,
    )
    .await?;

    let span = tracing::Span::current();
    if let (Some(tx_id), Some(psbt)) = (tx_id, psbt) {
        span.record("txid", &tracing::field::display(tx_id));
        span.record("psbt", &tracing::field::display(&psbt));

        let wallet_ids = wallet_totals.keys().copied().collect();
        span.record("batch_id", &tracing::field::display(data.batch_id));
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
        utxos
            .reserve_utxos_in_batch(&mut tx, data.account_id, batch.id, included_utxos)
            .await?;

        let batch_id = batch.id;
        batches.create_in_tx(&mut tx, batch).await?;

        for id in included_payouts
            .into_values()
            .flat_map(|payouts| payouts.into_iter().map(|(id, _, _)| id))
        {
            unbatched_payouts.mark_used(batch_id, id);
        }
        payouts
            .update_unbatched(&mut tx, batch_id, unbatched_payouts)
            .await?;

        Ok((data, Some((tx, wallet_ids))))
    } else {
        Ok((data, None))
    }
}

pub async fn construct_psbt(
    pool: sqlx::Pool<sqlx::Postgres>,
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    unbatched_payouts: &UnbatchedPayouts,
    utxos: &Utxos,
    wallets: Wallets,
    payout_queue: PayoutQueue,
) -> Result<FinishedPsbtBuild, BriaError> {
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
    let keychain_ids = wallets.values().flat_map(|w| w.keychain_ids());

    let reserved_utxos = utxos
        .outpoints_bdk_should_not_select(tx, keychain_ids)
        .await?;
    span.record(
        "n_reserved_utxos",
        reserved_utxos.values().fold(0, |acc, v| acc + v.len()),
    );

    let tx_payouts = unbatched_payouts.into_tx_payouts();
    let fee_rate =
        crate::fee_estimation::MempoolSpaceClient::fee_rate(queue_cfg.tx_priority).await?;

    PsbtBuilder::construct_psbt(
        &pool,
        queue_cfg.consolidate_deprecated_keychains,
        fee_rate,
        reserved_utxos,
        tx_payouts,
        wallets,
    )
    .await
}

impl From<WalletTotals> for WalletSummary {
    fn from(wt: WalletTotals) -> Self {
        Self {
            wallet_id: wt.wallet_id,
            signing_keychains: wt.keychains_with_inputs,
            total_in_sats: wt.input_satoshis,
            total_spent_sats: wt.output_satoshis,
            fee_sats: wt.fee_satoshis,
            change_sats: wt.change_satoshis,
            change_address: wt.change_outpoint.map(|_| wt.change_address.address),
            change_outpoint: wt.change_outpoint,
            current_keychain_id: wt.change_keychain_id,
            batch_created_ledger_tx_id: None,
            batch_broadcast_ledger_tx_id: None,
        }
    }
}
