use bdk::blockchain::{ElectrumBlockchain, GetHeight};
use electrum_client::{Client, ConfigBuilder};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    app::BlockchainConfig,
    bdk::pg::{ConfirmedIncomeUtxo, Transactions, Utxos as BdkUtxos},
    error::*,
    ledger::*,
    primitives::*,
    utxo::Utxos,
    wallet::*,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncWalletData {
    pub(super) wallet_id: WalletId,
}

impl SyncWalletData {
    pub fn new(id: WalletId) -> Self {
        SyncWalletData { wallet_id: id }
    }
}

#[instrument(
    name = "job.sync_wallet",
    skip(pool, wallets, bria_utxos, ledger),
    fields(n_pending_utxos, n_confirmed_utxos, n_found_txs),
    err
)]
pub async fn execute(
    pool: sqlx::PgPool,
    wallets: Wallets,
    blockchain_cfg: BlockchainConfig,
    bria_utxos: Utxos,
    ledger: Ledger,
    data: SyncWalletData,
) -> Result<SyncWalletData, BriaError> {
    let span = tracing::Span::current();
    let wallet = wallets.find_by_id(data.wallet_id).await?;
    let mut n_pending_utxos = 0;
    let mut n_confirmed_utxos = 0;
    let mut n_found_txs = 0;
    let mut utxos_to_fetch = HashMap::new();
    let mut income_bria_utxos = Vec::new();
    for keychain_wallet in wallet.keychain_wallets(pool.clone()) {
        let keychain_id = keychain_wallet.keychain_id;
        utxos_to_fetch.clear();
        utxos_to_fetch.insert(keychain_id, Vec::<bitcoin::OutPoint>::new());
        let blockchain = ElectrumBlockchain::from(
            Client::from_config(
                &blockchain_cfg.electrum_url,
                ConfigBuilder::new()
                    .retry(10)
                    .timeout(Some(4))
                    .expect("couldn't set electrum timeout")
                    .build(),
            )
            .unwrap(),
        );
        let current_height = blockchain.get_height()?;
        let _ = keychain_wallet.sync(blockchain).await;
        let bdk_txs = Transactions::new(keychain_id, pool.clone());
        let bdk_utxos = BdkUtxos::new(keychain_id, pool.clone());
        let mut txs_to_skip = Vec::new();
        while let Ok(Some(mut unsynced_tx)) = bdk_txs.find_unsynced_tx(&txs_to_skip).await {
            income_bria_utxos.clear();
            n_found_txs += 1;
            span.record("n_found_txs", n_found_txs);
            let mut change = Vec::new();
            let n_inputs = {
                let inputs = utxos_to_fetch.get_mut(&keychain_id).unwrap();
                inputs.clear();
                for input in unsynced_tx.inputs {
                    inputs.push(input.0.outpoint);
                }
                inputs.len()
            };
            if n_inputs > 0 {
                income_bria_utxos = bria_utxos
                    .get_pending_ledger_tx_ids_for_utxos(&utxos_to_fetch)
                    .await?;
                if income_bria_utxos.len() != n_inputs {
                    txs_to_skip.push(unsynced_tx.tx_id.to_string());
                    continue;
                }
            }
            txs_to_skip.clear();
            for output in unsynced_tx.outputs.drain(..) {
                if output.0.keychain == bitcoin::KeychainKind::Internal {
                    change.push(output);
                    continue;
                }
                let (local_utxo, path) = output;
                let address_info = keychain_wallet
                    .find_address_from_path(path, local_utxo.keychain)
                    .await?;
                let mut tx = pool.begin().await?;
                if let Some(pending_id) = bria_utxos
                    .new_utxo(
                        &mut tx,
                        wallet.id,
                        keychain_id,
                        &address_info,
                        &local_utxo,
                        unsynced_tx.sats_per_vbyte_when_created,
                    )
                    .await?
                {
                    n_pending_utxos += 1;
                    let fee_rate =
                        crate::fee_estimation::MempoolSpaceClient::fee_rate(TxPriority::NextBlock)
                            .await?
                            .as_sat_per_vb();
                    let weight = keychain_wallet.max_satisfaction_weight().await?;
                    let fees = Satoshis::from((fee_rate as u64) * (weight as u64));
                    bdk_utxos.mark_as_synced(&mut tx, &local_utxo).await?;
                    ledger
                        .incoming_utxo(
                            tx,
                            pending_id,
                            IncomingUtxoParams {
                                journal_id: wallet.journal_id,
                                onchain_incoming_account_id: wallet
                                    .ledger_account_ids
                                    .onchain_incoming_id,
                                logical_incoming_account_id: wallet
                                    .ledger_account_ids
                                    .logical_incoming_id,
                                onchain_fee_account_id: wallet.ledger_account_ids.fee_id,
                                meta: IncomingUtxoMeta {
                                    wallet_id: data.wallet_id,
                                    keychain_id,
                                    outpoint: local_utxo.outpoint,
                                    satoshis: local_utxo.txout.value.into(),
                                    address: address_info.address,
                                    encumbered_spending_fee_sats: fees,
                                    confirmation_time: unsynced_tx.confirmation_time.clone(),
                                },
                            },
                        )
                        .await?;
                }
            }
            bdk_txs.mark_as_synced(unsynced_tx.tx_id).await?;
        }

        loop {
            let mut tx = pool.begin().await?;
            let min_height = current_height - wallet.config.mark_confirmed_after_n_confs + 1;
            if let Ok(Some(ConfirmedIncomeUtxo {
                outpoint,
                spent,
                confirmation_time,
            })) = bdk_utxos
                .find_confirmed_income_utxo(&mut tx, min_height)
                .await
            {
                let utxo = bria_utxos
                    .confirm_utxo(
                        &mut tx,
                        keychain_id,
                        outpoint,
                        spent,
                        confirmation_time.height,
                    )
                    .await?;
                n_confirmed_utxos += 1;

                ledger
                    .confirmed_utxo(
                        tx,
                        utxo.confirmed_ledger_tx_id,
                        ConfirmedUtxoParams {
                            journal_id: wallet.journal_id,
                            ledger_account_ids: wallet.ledger_account_ids,
                            pending_id: utxo.pending_ledger_tx_id,
                            meta: ConfirmedUtxoMeta {
                                wallet_id: data.wallet_id,
                                keychain_id,
                                confirmation_time,
                                satoshis: utxo.value,
                                outpoint,
                                address: utxo.address,
                            },
                        },
                    )
                    .await?;
            } else {
                break;
            }
        }
    }

    span.record("n_pending_utxos", n_pending_utxos);
    span.record("n_confirmed_utxos", n_confirmed_utxos);
    span.record("n_found_txs", n_found_txs);

    Ok(data)
}
