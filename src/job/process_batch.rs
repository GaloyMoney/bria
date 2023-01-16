use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::{app::BlockchainConfig, batch::*, error::*, ledger::*, primitives::*, wallet::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessBatchData {
    pub(super) account_id: AccountId,
    pub(super) batch_id: BatchId,
}

#[instrument(name = "job.process_batch", skip(pool), err)]
pub async fn execute(
    pool: sqlx::PgPool,
    data: ProcessBatchData,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
    wallets: Wallets,
    batches: Batches,
) -> Result<ProcessBatchData, BriaError> {
    // load psbt
    // for each keychain
    // for each xpub
    // load signer

    let Batch {
        id,
        bitcoin_tx_id,
        batch_group_id,
        wallet_summaries,
    } = batches.find_by_id(data.batch_id).await?;

    for (wallet_id, wallet_summary) in wallet_summaries.into_iter() {
        let tx = pool.begin().await?;
        let wallet = wallets.find_by_id(wallet_id).await?;

        ledger
            .create_batch(
                tx,
                CreateBatchParams {
                    journal_id: wallet.journal_id,
                    outgoing_ledger_account_id: wallet.ledger_account_ids.outgoing_id,
                    at_rest_ledger_account_id: wallet.ledger_account_ids.at_rest_id,
                    satoshis: wallet_summary.total_out_sats,
                    external_id: Uuid::new_v4(), // TODO: Some hash of format!("{}-{}", bitcoin_tx_id, wallet_id)
                    meta: CreateBatchMeta {
                        batch_id: id,
                        batch_group_id,
                        bitcoin_tx_id,
                    },
                },
            )
            .await?;
    }

    Ok(data)
}
