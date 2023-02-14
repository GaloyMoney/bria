use serde::{Deserialize, Serialize};
use tracing::instrument;

use std::collections::HashSet;

use crate::{
    batch::*, batch_group::*, bdk::pg::Utxos, error::*, payout::*, primitives::*, wallet::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessBatchGroupData {
    pub(super) batch_group_id: BatchGroupId,
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
}

impl ProcessBatchGroupData {
    pub fn new(batch_group_id: BatchGroupId, account_id: AccountId) -> Self {
        Self {
            batch_group_id,
            account_id,
            batch_id: BatchId::new(),
        }
    }
}

#[instrument(
    name = "job.process_batch_group",
    skip_all,
    fields(
        n_unbatched_payouts,
        batch_group_name,
        n_reserved_utxos,
        txid,
        psbt,
        batch_id,
        batch_group_id
    ),
    err
)]
#[allow(clippy::type_complexity)]
pub async fn execute<'a>(
    pool: sqlx::PgPool,
    payouts: Payouts,
    wallets: Wallets,
    batch_groups: BatchGroups,
    batches: Batches,
    data: ProcessBatchGroupData,
) -> Result<
    (
        ProcessBatchGroupData,
        Option<(sqlx::Transaction<'a, sqlx::Postgres>, Vec<WalletId>)>,
    ),
    BriaError,
> {
    let span = tracing::Span::current();
    let BatchGroup {
        config: bg_cfg,
        name,
        ..
    } = batch_groups.find_by_id(data.batch_group_id).await?;
    span.record("batch_group_name", name);
    span.record(
        "batch_group_id",
        &tracing::field::display(data.batch_group_id),
    );

    let unbatched_payouts = payouts.list_unbatched(data.batch_group_id).await?;
    span.record(
        "n_unbatched_payouts",
        unbatched_payouts.values().fold(0, |acc, v| acc + v.len()),
    );

    let wallet_ids = unbatched_payouts.keys().copied().collect();
    let mut wallets = wallets.find_by_ids(wallet_ids).await?;
    let keychain_ids: HashSet<KeychainId> = wallets
        .values()
        .flat_map(|w| w.keychains.iter().map(|(id, _)| *id))
        .collect();

    let mut tx = pool.begin().await?;
    let all_utxos = Utxos::new(KeychainId::new(), pool.clone());
    let reserved_utxos = all_utxos
        .list_reserved_unspent_utxos(&mut tx, keychain_ids)
        .await?;
    span.record(
        "n_reserved_utxos",
        reserved_utxos.values().fold(0, |acc, v| acc + v.len()),
    );
    let fee_rate = crate::fee_estimation::MempoolSpaceClient::fee_rate(bg_cfg.tx_priority).await?;

    let mut outer_builder = PsbtBuilder::new()
        .consolidate_deprecated_keychains(bg_cfg.consolidate_deprecated_keychains)
        .fee_rate(fee_rate)
        .reserved_utxos(reserved_utxos)
        .accept_wallets();

    for (wallet_id, payouts) in unbatched_payouts {
        let wallet = wallets.remove(&wallet_id).expect("Wallet not found");

        let mut builder = outer_builder.wallet_payouts(wallet.id, payouts);
        for keychain in wallet.deprecated_keychain_wallets(pool.clone()) {
            builder = keychain.dispatch_bdk_wallet(builder).await?;
        }
        outer_builder = wallet
            .current_keychain_wallet(&pool)
            .dispatch_bdk_wallet(builder.accept_current_keychain())
            .await?
            .next_wallet();
    }

    let FinishedPsbtBuild {
        psbt,
        included_payouts,
        included_utxos,
        wallet_totals,
        tx_id,
        fee_satoshis,
        ..
    } = outer_builder.finish();

    if let (Some(tx_id), Some(psbt)) = (tx_id, psbt) {
        span.record("txid", &tracing::field::display(tx_id));
        span.record("psbt", &tracing::field::display(&psbt));

        let wallet_ids = wallet_totals.keys().copied().collect();
        span.record("batch_id", &tracing::field::display(data.batch_id));
        let batch = NewBatch::builder()
            .id(data.batch_id)
            .batch_group_id(data.batch_group_id)
            .tx_id(tx_id)
            .unsigned_psbt(psbt)
            .total_fee_sats(fee_satoshis)
            .included_payouts(
                included_payouts
                    .into_iter()
                    .map(|(wallet_id, payouts)| {
                        (wallet_id, payouts.into_iter().map(|p| p.id).collect())
                    })
                    .collect(),
            )
            .included_utxos(included_utxos)
            .wallet_summaries(
                wallet_totals
                    .into_iter()
                    .map(|(wallet_id, total)| (wallet_id, total.into()))
                    .collect(),
            )
            .build()
            .expect("Couldn't build batch");

        all_utxos
            .reserve_utxos(
                &mut tx,
                batch.id,
                batch.iter_utxos().map(|(_, k, utxo)| (k, utxo)),
            )
            .await?;
        batches.create_in_tx(&mut tx, batch).await?;

        Ok((data, Some((tx, wallet_ids))))
    } else {
        Ok((data, None))
    }
}

impl From<WalletTotals> for WalletSummary {
    fn from(wt: WalletTotals) -> Self {
        Self {
            wallet_id: wt.wallet_id,
            total_in_sats: wt.input_satoshis,
            total_out_sats: wt.output_satoshis,
            fee_sats: wt.fee_satoshis,
            change_sats: wt.change_satoshis,
            change_address: wt.change_address.address,
            ledger_tx_pending_id: sqlx_ledger::TransactionId::new(),
            ledger_tx_settled_id: sqlx_ledger::TransactionId::new(),
        }
    }
}
