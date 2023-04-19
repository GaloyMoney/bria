use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

use crate::{
    app::BlockchainConfig, batch::*, error::*, ledger::*, primitives::*, utxo::*, wallet::*,
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
) -> Result<BatchWalletAccountingData, BriaError> {
    let Batch {
        id,
        bitcoin_tx_id,
        batch_group_id,
        wallet_summaries,
        included_utxos,
        ..
    } = batches.find_by_id(data.account_id, data.batch_id).await?;

    let wallet_summary = wallet_summaries
        .get(&data.wallet_id)
        .expect("wallet summary not found");
    let wallet = wallets.find_by_id(data.wallet_id).await?;

    let utxos = included_utxos
        .get(&data.wallet_id)
        .expect("utxos not found");
    let utxos = bria_utxos.list_utxos_by_outpoint(utxos).await?;
    let reserved_fees = ledger
        .sum_reserved_fees_in_txs(utxos.into_iter().map(|u| u.pending_income_ledger_tx_id))
        .await?;

    if let Some((tx, tx_id)) = batches
        .set_create_batch_ledger_tx_id(data.batch_id, data.wallet_id)
        .await?
    {
        ledger
            .create_batch(
                tx,
                tx_id,
                CreateBatchParams {
                    journal_id: wallet.journal_id,
                    ledger_account_ids: wallet.ledger_account_ids,
                    fee_sats: wallet_summary.fee_sats,
                    total_spent_sats: wallet_summary.total_spent_sats,
                    total_in_sats: wallet_summary.total_in_sats,
                    correlation_id: Uuid::from(data.batch_id),
                    reserved_fees,
                    meta: CreateBatchMeta {
                        batch_id: id,
                        batch_group_id,
                        bitcoin_tx_id,
                        change_keychain_id: wallet_summary.change_keychain_id,
                        change_outpoint: wallet_summary.change_outpoint,
                        change_address: wallet_summary
                            .change_outpoint
                            .as_ref()
                            .map(|_| wallet_summary.change_address.clone()),
                    },
                },
            )
            .await?;
    }
    Ok(data)
}
