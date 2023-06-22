use bdk::blockchain::{ElectrumBlockchain, GetHeight};
use electrum_client::{Client, ConfigBuilder};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

use super::error::JobError;
use crate::{
    address::*,
    app::BlockchainConfig,
    batch::*,
    bdk::error::BdkError,
    bdk::pg::{ConfirmedIncomeUtxo, ConfirmedSpendTransaction, Transactions, Utxos as BdkUtxos},
    fees::{self, MempoolSpaceClient},
    ledger::*,
    primitives::*,
    utxo::{Utxos, WalletUtxo},
    wallet::*,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncWalletData {
    pub(super) account_id: AccountId,
    pub(super) wallet_id: WalletId,
}

impl SyncWalletData {
    pub fn new(account_id: AccountId, wallet_id: WalletId) -> Self {
        SyncWalletData {
            account_id,
            wallet_id,
        }
    }
}

struct InstrumentationTrackers {
    n_pending_utxos: usize,
    n_confirmed_utxos: usize,
    n_found_txs: usize,
}
impl InstrumentationTrackers {
    fn new() -> Self {
        InstrumentationTrackers {
            n_pending_utxos: 0,
            n_confirmed_utxos: 0,
            n_found_txs: 0,
        }
    }
}

struct Deps {
    blockchain_cfg: BlockchainConfig,
    bria_addresses: Addresses,
    bria_utxos: Utxos,
    ledger: Ledger,
}

const MAX_TXS_PER_SYNC: usize = 100;

#[instrument(
    name = "job.sync_wallet",
    skip(pool, wallets, batches, bria_utxos, bria_addresses, ledger),
    fields(
        n_pending_utxos,
        n_confirmed_utxos,
        n_found_txs,
        has_more,
        current_height
    ),
    err
)]
#[allow(clippy::too_many_arguments)]
pub async fn execute(
    pool: sqlx::PgPool,
    wallets: Wallets,
    blockchain_cfg: BlockchainConfig,
    bria_utxos: Utxos,
    bria_addresses: Addresses,
    ledger: Ledger,
    batches: Batches,
    data: SyncWalletData,
    mempool_space_client: MempoolSpaceClient,
) -> Result<(bool, SyncWalletData), JobError> {
    info!("Starting sync_wallet job: {:?}", data);
    let span = tracing::Span::current();
    let wallet = wallets.find_by_id(data.wallet_id).await?;
    let mut trackers = InstrumentationTrackers::new();
    let deps = Deps {
        blockchain_cfg,
        bria_addresses,
        bria_utxos,
        ledger,
    };
    let mut utxos_to_fetch = HashMap::new();
    let mut income_bria_utxos = Vec::new();
    for keychain_wallet in wallet.keychain_wallets(pool.clone()) {
        info!("Syncing keychain '{}'", keychain_wallet.keychain_id);
        let fees_to_encumber = fees::fees_to_encumber(
            &mempool_space_client,
            keychain_wallet.max_satisfaction_weight(),
        )
        .await?;
        let keychain_id = keychain_wallet.keychain_id;
        utxos_to_fetch.clear();
        utxos_to_fetch.insert(keychain_id, Vec::<bitcoin::OutPoint>::new());
        let (blockchain, current_height) = init_electrum(&deps.blockchain_cfg.electrum_url).await?;
        span.record("current_height", current_height);
        let latest_change_settle_height = wallet.config.latest_change_settle_height(current_height);
        keychain_wallet.sync(blockchain).await?;
        let bdk_txs = Transactions::new(keychain_id, pool.clone());
        let bdk_utxos = BdkUtxos::new(keychain_id, pool.clone());
        let mut txs_to_skip = Vec::new();
        info!(
            "Sync via bdk for keychain '{}'",
            keychain_wallet.keychain_id
        );
        while let Ok(Some(mut unsynced_tx)) = bdk_txs.find_unsynced_tx(&txs_to_skip).await {
            tracing::info!(?unsynced_tx);
            income_bria_utxos.clear();
            trackers.n_found_txs += 1;
            let mut change = Vec::new();
            let n_inputs = {
                let inputs = utxos_to_fetch.get_mut(&keychain_id).unwrap();
                inputs.clear();
                for input in unsynced_tx.inputs {
                    inputs.push(input.0.outpoint);
                }
                inputs.len()
            };
            let spend_tx = n_inputs > 0;
            if spend_tx {
                income_bria_utxos = deps
                    .bria_utxos
                    .list_utxos_by_outpoint(&utxos_to_fetch)
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
                let found_addr = NewAddress::builder()
                    .account_id(data.account_id)
                    .wallet_id(data.wallet_id)
                    .keychain_id(keychain_id)
                    .address(address_info.address.clone())
                    .kind(address_info.keychain)
                    .address_idx(address_info.index)
                    .metadata(Some(address_metadata(&unsynced_tx.tx_id)))
                    .build()
                    .expect("Could not build new address in sync wallet");
                if let Some((pending_id, mut tx)) = deps
                    .bria_utxos
                    .new_utxo_detected(
                        data.account_id,
                        wallet.id,
                        keychain_id,
                        &address_info,
                        &local_utxo,
                        unsynced_tx.sats_per_vbyte_when_created,
                        spend_tx,
                    )
                    .await?
                {
                    trackers.n_pending_utxos += 1;
                    deps.bria_addresses
                        .persist_if_not_present(&mut tx, found_addr)
                        .await?;
                    bdk_utxos.mark_as_synced(&mut tx, &local_utxo).await?;
                    deps.ledger
                        .utxo_detected(
                            tx,
                            pending_id,
                            UtxoDetectedParams {
                                journal_id: wallet.journal_id,
                                onchain_incoming_account_id: wallet
                                    .ledger_account_ids
                                    .onchain_incoming_id,
                                effective_incoming_account_id: wallet
                                    .ledger_account_ids
                                    .effective_incoming_id,
                                onchain_fee_account_id: wallet.ledger_account_ids.fee_id,
                                meta: UtxoDetectedMeta {
                                    account_id: data.account_id,
                                    wallet_id: data.wallet_id,
                                    keychain_id,
                                    outpoint: local_utxo.outpoint,
                                    satoshis: local_utxo.txout.value.into(),
                                    address: address_info.address,
                                    encumbered_spending_fees: std::iter::once((
                                        local_utxo.outpoint,
                                        fees_to_encumber,
                                    ))
                                    .collect(),
                                    confirmation_time: unsynced_tx.confirmation_time.clone(),
                                },
                            },
                        )
                        .await?;
                    let conf_time = match unsynced_tx.confirmation_time.as_ref() {
                        Some(t)
                            if t.height
                                <= wallet.config.latest_settle_height(current_height, spend_tx) =>
                        {
                            Some(t)
                        }
                        _ => None,
                    };
                    if let Some(conf_time) = conf_time {
                        let mut tx = pool.begin().await?;
                        bdk_utxos.mark_confirmed(&mut tx, &local_utxo).await?;
                        let utxo = deps
                            .bria_utxos
                            .settle_utxo(
                                &mut tx,
                                keychain_id,
                                local_utxo.outpoint,
                                local_utxo.is_spent,
                                conf_time.height,
                            )
                            .await?;
                        trackers.n_confirmed_utxos += 1;

                        deps.ledger
                            .utxo_settled(
                                tx,
                                utxo.utxo_settled_ledger_tx_id,
                                UtxoSettledParams {
                                    journal_id: wallet.journal_id,
                                    ledger_account_ids: wallet.ledger_account_ids,
                                    pending_id: utxo.utxo_detected_ledger_tx_id,
                                    meta: UtxoSettledMeta {
                                        account_id: data.account_id,
                                        wallet_id: data.wallet_id,
                                        keychain_id,
                                        confirmation_time: conf_time.clone(),
                                        satoshis: utxo.value,
                                        outpoint: local_utxo.outpoint,
                                        address: utxo.address,
                                        already_spent_tx_id: utxo.spend_detected_ledger_tx_id,
                                    },
                                },
                            )
                            .await?;
                    }
                }
            }
            if spend_tx {
                let (mut tx, create_batch_tx_id, tx_id) =
                    if let Some((tx, create_batch_tx_id, tx_id)) = batches
                        .set_batch_broadcast_ledger_tx_id(unsynced_tx.tx_id, wallet.id)
                        .await?
                    {
                        (tx, Some(create_batch_tx_id), tx_id)
                    } else {
                        (pool.begin().await?, None, LedgerTransactionId::new())
                    };

                let mut change_utxos = Vec::new();
                for (utxo, path) in change.iter() {
                    let address_info = keychain_wallet
                        .find_address_from_path(*path, utxo.keychain)
                        .await?;
                    let found_addr = NewAddress::builder()
                        .account_id(data.account_id)
                        .wallet_id(data.wallet_id)
                        .keychain_id(keychain_id)
                        .address(address_info.address.clone())
                        .kind(address_info.keychain)
                        .address_idx(address_info.index)
                        .metadata(Some(address_metadata(&unsynced_tx.tx_id)))
                        .build()
                        .expect("Could not build new address in sync wallet");
                    deps.bria_addresses
                        .persist_if_not_present(&mut tx, found_addr)
                        .await?;
                    change_utxos.push((utxo, address_info));
                }

                if let Some((settled_sats, allocations)) = deps
                    .bria_utxos
                    .spend_detected(
                        &mut tx,
                        data.account_id,
                        wallet.id,
                        keychain_id,
                        tx_id,
                        income_bria_utxos
                            .iter()
                            .map(|WalletUtxo { outpoint, .. }| outpoint),
                        &change_utxos,
                        unsynced_tx.sats_per_vbyte_when_created,
                    )
                    .await?
                {
                    if let Some(create_batch_tx_id) = create_batch_tx_id {
                        deps.ledger
                            .batch_broadcast(
                                tx,
                                create_batch_tx_id,
                                tx_id,
                                fees_to_encumber,
                                wallet.ledger_account_ids,
                            )
                            .await?;
                    } else {
                        let reserved_fees = deps
                            .ledger
                            .sum_reserved_fees_in_txs(income_bria_utxos.iter().fold(
                                HashMap::new(),
                                |mut m, u| {
                                    m.entry(u.utxo_detected_ledger_tx_id)
                                        .or_insert_with(Vec::new)
                                        .push(u.outpoint);
                                    m
                                },
                            ))
                            .await?;
                        deps.ledger
                            .spend_detected(
                                tx,
                                tx_id,
                                SpendDetectedParams {
                                    journal_id: wallet.journal_id,
                                    ledger_account_ids: wallet.ledger_account_ids,
                                    reserved_fees,
                                    meta: SpendDetectedMeta {
                                        encumbered_spending_fees: change_utxos
                                            .iter()
                                            .map(|(u, _)| (u.outpoint, fees_to_encumber))
                                            .collect(),
                                        withdraw_from_effective_when_settled: allocations,
                                        tx_summary: WalletTransactionSummary {
                                            account_id: data.account_id,
                                            wallet_id: wallet.id,
                                            current_keychain_id: keychain_id,
                                            bitcoin_tx_id: unsynced_tx.tx_id,
                                            total_utxo_in_sats: unsynced_tx.total_utxo_in_sats,
                                            total_utxo_settled_in_sats: settled_sats,
                                            fee_sats: unsynced_tx.fee_sats,
                                            change_utxos: change_utxos
                                                .iter()
                                                .map(|(u, a)| ChangeOutput {
                                                    outpoint: u.outpoint,
                                                    address: a.address.clone(),
                                                    satoshis: Satoshis::from(u.txout.value),
                                                })
                                                .collect(),
                                        },
                                        confirmation_time: unsynced_tx.confirmation_time.clone(),
                                    },
                                },
                            )
                            .await?;
                    }
                }
            }
            bdk_txs.mark_as_synced(unsynced_tx.tx_id).await?;
            if let Some(conf_time) = unsynced_tx.confirmation_time {
                if !spend_tx || conf_time.height > latest_change_settle_height {
                    continue;
                }
                let mut tx = pool.begin().await?;
                if let Some((pending_out_id, confirmed_out_id, change_spent)) = deps
                    .bria_utxos
                    .spend_settled(
                        &mut tx,
                        keychain_id,
                        utxos_to_fetch.get(&keychain_id).unwrap().iter(),
                        change.get(0).as_ref().map(|(u, _)| u.clone()),
                        conf_time.height,
                    )
                    .await?
                {
                    bdk_txs.mark_confirmed(&mut tx, unsynced_tx.tx_id).await?;
                    deps.ledger
                        .spend_settled(
                            tx,
                            confirmed_out_id,
                            wallet.journal_id,
                            wallet.ledger_account_ids,
                            pending_out_id,
                            conf_time,
                            change_spent,
                        )
                        .await?;
                }
            }
            if trackers.n_found_txs >= MAX_TXS_PER_SYNC {
                break;
            }
        }

        loop {
            let mut tx = pool.begin().await?;
            let min_height = wallet.config.latest_income_settle_height(current_height);
            if let Ok(Some(ConfirmedIncomeUtxo {
                outpoint,
                spent,
                confirmation_time,
            })) = bdk_utxos
                .find_confirmed_income_utxo(&mut tx, min_height)
                .await
            {
                let utxo = deps
                    .bria_utxos
                    .settle_utxo(
                        &mut tx,
                        keychain_id,
                        outpoint,
                        spent,
                        confirmation_time.height,
                    )
                    .await?;
                trackers.n_confirmed_utxos += 1;

                deps.ledger
                    .utxo_settled(
                        tx,
                        utxo.utxo_settled_ledger_tx_id,
                        UtxoSettledParams {
                            journal_id: wallet.journal_id,
                            ledger_account_ids: wallet.ledger_account_ids,
                            pending_id: utxo.utxo_detected_ledger_tx_id,
                            meta: UtxoSettledMeta {
                                account_id: data.account_id,
                                wallet_id: data.wallet_id,
                                keychain_id,
                                confirmation_time,
                                satoshis: utxo.value,
                                outpoint,
                                address: utxo.address,
                                already_spent_tx_id: utxo.spend_detected_ledger_tx_id,
                            },
                        },
                    )
                    .await?;
            } else {
                break;
            }
        }

        loop {
            let mut tx = pool.begin().await?;
            let min_height = wallet.config.latest_change_settle_height(current_height);
            if let Ok(Some(ConfirmedSpendTransaction {
                confirmation_time,
                inputs,
                outputs,
                ..
            })) = bdk_txs.find_confirmed_spend_tx(&mut tx, min_height).await
            {
                let change_utxo = outputs
                    .into_iter()
                    .find(|u| u.keychain == bitcoin::KeychainKind::Internal);
                if let Some((pending_out_id, confirmed_out_id, change_spent)) = deps
                    .bria_utxos
                    .spend_settled(
                        &mut tx,
                        keychain_id,
                        inputs.iter().map(|u| &u.outpoint),
                        change_utxo,
                        confirmation_time.height,
                    )
                    .await?
                {
                    deps.ledger
                        .spend_settled(
                            tx,
                            confirmed_out_id,
                            wallet.journal_id,
                            wallet.ledger_account_ids,
                            pending_out_id,
                            confirmation_time,
                            change_spent,
                        )
                        .await?;
                }
            } else {
                break;
            }
        }

        loop {
            let mut tx = pool.begin().await?;
            if let Some((outpoint, keychain_id)) = bdk_utxos.find_delete_unsynced(&mut tx).await? {

                bdk_txs.delete_transaction(&mut tx, outpoint).await?;
                if let Some(detected_txn_id) = deps
                    .bria_utxos
                    .delete_unsynced_utxo(&mut tx, outpoint, keychain_id)
                    .await?
                {
                    deps.ledger
                        .utxo_dropped(tx, LedgerTransactionId::new(), detected_txn_id)
                        .await?
                }
            } else {
                break;
            }
        }
    }

    let has_more = trackers.n_found_txs >= MAX_TXS_PER_SYNC;
    span.record("n_pending_utxos", trackers.n_pending_utxos);
    span.record("n_confirmed_utxos", trackers.n_confirmed_utxos);
    span.record("n_found_txs", trackers.n_found_txs);
    span.record("has_more", has_more);

    Ok((has_more, data))
}

async fn init_electrum(electrum_url: &str) -> Result<(ElectrumBlockchain, u32), BdkError> {
    let blockchain = ElectrumBlockchain::from(Client::from_config(
        electrum_url,
        ConfigBuilder::new()
            .retry(10)
            .timeout(Some(60))
            .expect("couldn't set electrum timeout")
            .build(),
    )?);
    let current_height = blockchain.get_height()?;
    Ok((blockchain, current_height))
}

fn address_metadata(tx_id: &bitcoin::Txid) -> serde_json::Value {
    serde_json::json! {
        {
            "synced_in_tx": tx_id.to_string(),
        }
    }
}
