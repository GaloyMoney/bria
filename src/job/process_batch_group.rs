use bdk::LocalUtxo;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use std::collections::HashMap;

use crate::{app::BlockchainConfig, batch_group::*, error::*, payout::*, primitives::*, wallet::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessBatchGroupData {
    pub batch_group_id: BatchGroupId,
    pub account_id: AccountId,
    batch_id: BatchId,
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
    skip(pool, payouts, wallets, batch_groups),
    err
)]
pub async fn execute(
    pool: sqlx::PgPool,
    payouts: Payouts,
    wallets: Wallets,
    blockchain_cfg: BlockchainConfig,
    batch_groups: BatchGroups,
    data: ProcessBatchGroupData,
) -> Result<ProcessBatchGroupData, BriaError> {
    let BatchGroup { config: bg_cfg, .. } = batch_groups.find_by_id(data.batch_group_id).await?;

    let unbatched_payouts = payouts.list_unbatched(data.batch_group_id).await?;
    let wallet_ids = unbatched_payouts.keys().copied().collect();
    let mut wallets = wallets.find_by_ids(wallet_ids).await?;

    let fee_rate = crate::fee_estimation::MempoolSpaceClient::fee_rate(bg_cfg.tx_priority).await?;

    let mut outer_builder = PsbtBuilder::new()
        .consolidate_deprecated_keychains(bg_cfg.consolidate_deprecated_keychains)
        .fee_rate(fee_rate)
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
        ..
    } = outer_builder.finish();

    Ok(data)
}
