use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::{
    app::BlockchainConfig, batch::*, bdk::pg::Utxos, error::*, ledger::*, primitives::*, wallet::*,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchWalletAccountingData {
    pub(super) account_id: AccountId,
    pub(super) wallet_id: WalletId,
    pub(super) batch_id: BatchId,
}

#[instrument(
    name = "job.batch_wallet_accounting",
    skip(pool, wallets, batches, ledger),
    err
)]
pub async fn execute(
    pool: sqlx::PgPool,
    data: BatchWalletAccountingData,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
    wallets: Wallets,
    batches: Batches,
) -> Result<BatchWalletAccountingData, BriaError> {
    let Batch {
        id,
        bitcoin_tx_id,
        batch_group_id,
        wallet_summaries,
    } = batches.find_by_id(data.batch_id).await?;

    let wallet_summary = wallet_summaries
        .get(&data.wallet_id)
        .expect("wallet summary not found");
    let wallet = wallets.find_by_id(data.wallet_id).await?;

    match ledger
        .create_batch(CreateBatchParams {
            journal_id: wallet.journal_id,
            ledger_account_ids: wallet.ledger_account_ids,
            fee_sats: wallet_summary.fee_sats,
            satoshis: wallet_summary.total_out_sats,
            correlation_id: Uuid::from(data.batch_id),
            external_id: wallet_summary.ledger_tx_pending_id.to_string(),
            meta: CreateBatchMeta {
                batch_id: id,
                batch_group_id,
                bitcoin_tx_id,
            },
        })
        .await
    {
        Err(BriaError::SqlxLedger(sqlx_ledger::SqlxLedgerError::DuplicateKey(_))) => (),
        Err(e) => return Err(e.into()),
        Ok(_) => (),
    };

    let mut tx = pool.begin().await?;

    let utxos = batches.get_included_utxos(id, wallet.id).await?;
    let settled_utxos = Utxos::get_settled_utxos(&mut tx, &utxos).await?;

    Ok(data)
}
