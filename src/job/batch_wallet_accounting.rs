use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::instrument;

use super::error::JobError;
use crate::{
    app::BlockchainConfig, batch::*, ledger::*, payout::*, primitives::*, utxo::*, wallet::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchWalletAccountingData {
    pub(super) account_id: AccountId,
    pub(super) wallet_id: WalletId,
    pub(super) batch_id: BatchId,
    #[serde(flatten)]
    pub(super) tracing_data: HashMap<String, String>,
}

#[instrument(
    name = "job.batch_wallet_accounting",
    skip(wallets, batches, ledger, bria_utxos),
    err
)]
pub async fn execute(
    data: BatchWalletAccountingData,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
    wallets: Wallets,
    bria_utxos: Utxos,
    batches: Batches,
    payouts: Payouts,
) -> Result<BatchWalletAccountingData, JobError> {
    let Batch {
        id,
        bitcoin_tx_id,
        payout_queue_id,
        mut wallet_summaries,
        ..
    } = batches.find_by_id(data.account_id, data.batch_id).await?;

    let wallet_summary = wallet_summaries
        .remove(&data.wallet_id)
        .expect("wallet summary not found");
    let wallet = wallets.find_by_id(data.wallet_id).await?;

    let encumbered_fees = ledger
        .sum_reserved_fees_in_txs(
            bria_utxos
                .income_detected_ids_for_utxos_in(data.batch_id, data.wallet_id)
                .await?,
        )
        .await?;

    let payouts = payouts
        .list_for_batch(data.account_id, data.batch_id, data.wallet_id)
        .await?;
    if let Some((tx, tx_id)) = batches
        .set_batch_created_ledger_tx_id(data.batch_id, data.wallet_id)
        .await?
    {
        let mut change_utxos = Vec::new();
        if let (Some(outpoint), Some(address)) = (
            wallet_summary.change_outpoint,
            wallet_summary.change_address,
        ) {
            change_utxos.push(ChangeOutput {
                outpoint,
                address,
                satoshis: wallet_summary.change_sats,
            });
        }
        ledger
            .batch_created(
                tx,
                tx_id,
                BatchCreatedParams {
                    journal_id: wallet.journal_id,
                    ledger_account_ids: wallet.ledger_account_ids,
                    encumbered_fees,
                    meta: BatchCreatedMeta {
                        batch_info: BatchWalletInfo {
                            account_id: data.account_id,
                            wallet_id: data.wallet_id,
                            batch_id: id,
                            payout_queue_id,
                            included_payouts: payouts.into_iter().map(PayoutInfo::from).collect(),
                        },
                        tx_summary: WalletTransactionSummary {
                            account_id: data.account_id,
                            wallet_id: wallet_summary.wallet_id,
                            current_keychain_id: wallet_summary.current_keychain_id,
                            fee_sats: wallet_summary.fee_sats,
                            bitcoin_tx_id,
                            total_utxo_in_sats: wallet_summary.total_in_sats,
                            total_utxo_settled_in_sats: wallet_summary.total_in_sats,
                            change_utxos,
                        },
                    },
                },
            )
            .await?;
    }
    Ok(data)
}

impl From<Payout> for PayoutInfo {
    fn from(payout: Payout) -> Self {
        Self {
            id: payout.id,
            profile_id: payout.profile_id,
            satoshis: payout.satoshis,
            destination: payout.destination,
        }
    }
}
