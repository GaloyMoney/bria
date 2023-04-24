use bdk::blockchain::{ElectrumBlockchain, GetHeight};
use electrum_client::{Client, ConfigBuilder};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    address::*,
    app::BlockchainConfig,
    batch::*,
    bdk::pg::{ConfirmedIncomeUtxo, ConfirmedSpendTransaction, Transactions, Utxos as BdkUtxos},
    error::*,
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

#[instrument(
    name = "job.sync_wallet",
    skip(pool, wallets, batches, bria_utxos, bria_addresses, ledger),
    fields(n_pending_utxos, n_confirmed_utxos, n_found_txs),
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
) -> Result<SyncWalletData, BriaError> {
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
        let fees_to_encumber = fees_for_keychain(&keychain_wallet).await?;
        let keychain_id = keychain_wallet.keychain_id;
        utxos_to_fetch.clear();
        utxos_to_fetch.insert(keychain_id, Vec::<bitcoin::OutPoint>::new());
        let (blockchain, current_height) = init_electrum(&deps.blockchain_cfg.electrum_url).await?;
        let latest_change_settle_height = wallet.config.latest_change_settle_height(current_height);
        let _ = keychain_wallet.sync(blockchain).await;
        let bdk_txs = Transactions::new(keychain_id, pool.clone());
        let bdk_utxos = BdkUtxos::new(keychain_id, pool.clone());
        let mut txs_to_skip = Vec::new();
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
                    .new_income_utxo(
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
                                    encumbered_spending_fee_sats: fees_to_encumber,
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
                            .confirm_utxo(
                                &mut tx,
                                keychain_id,
                                local_utxo.outpoint,
                                local_utxo.is_spent,
                                conf_time.height,
                            )
                            .await?;
                        trackers.n_confirmed_utxos += 1;

                        deps.ledger
                            .confirmed_utxo(
                                tx,
                                utxo.confirmed_income_ledger_tx_id,
                                ConfirmedUtxoParams {
                                    journal_id: wallet.journal_id,
                                    ledger_account_ids: wallet.ledger_account_ids,
                                    pending_id: utxo.pending_income_ledger_tx_id,
                                    meta: ConfirmedUtxoMeta {
                                        wallet_id: data.wallet_id,
                                        keychain_id,
                                        confirmation_time: conf_time.clone(),
                                        satoshis: utxo.value,
                                        outpoint: local_utxo.outpoint,
                                        address: utxo.address,
                                        already_spent_tx_id: utxo.pending_spend_ledger_tx_id,
                                    },
                                },
                            )
                            .await?;
                    }
                }
            }
            if spend_tx {
                let (change_utxo, found_addr) = if !change.is_empty() {
                    let (utxo, path) = change[0].clone();
                    let address_info = keychain_wallet
                        .find_address_from_path(path, utxo.keychain)
                        .await?;
                    let found_address = NewAddress::builder()
                        .account_id(data.account_id)
                        .wallet_id(data.wallet_id)
                        .keychain_id(keychain_id)
                        .address(address_info.address.clone())
                        .kind(address_info.keychain)
                        .address_idx(address_info.index)
                        .metadata(Some(address_metadata(&unsynced_tx.tx_id)))
                        .build()
                        .expect("Could not build new address in sync wallet");
                    (Some((utxo, address_info)), Some(found_address))
                } else {
                    (None, None)
                };
                let (mut tx, create_batch_tx_id, tx_id) =
                    if let Some((tx, create_batch_tx_id, tx_id)) = batches
                        .set_submitted_ledger_tx_id(unsynced_tx.tx_id, wallet.id)
                        .await?
                    {
                        (tx, Some(create_batch_tx_id), tx_id)
                    } else {
                        (pool.begin().await?, None, LedgerTransactionId::new())
                    };
                if let Some((settled_sats, allocations)) = deps
                    .bria_utxos
                    .mark_spent(
                        &mut tx,
                        wallet.id,
                        keychain_id,
                        tx_id,
                        income_bria_utxos
                            .iter()
                            .map(|WalletUtxo { outpoint, .. }| outpoint),
                        change_utxo.as_ref(),
                        unsynced_tx.sats_per_vbyte_when_created,
                    )
                    .await?
                {
                    if let Some(found_addr) = found_addr {
                        deps.bria_addresses
                            .persist_if_not_present(&mut tx, found_addr)
                            .await?;
                    }
                    if let Some(create_batch_tx_id) = create_batch_tx_id {
                        deps.ledger
                            .submit_batch(
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
                            .sum_reserved_fees_in_txs(
                                income_bria_utxos
                                    .iter()
                                    .map(|u| u.pending_income_ledger_tx_id),
                            )
                            .await?;
                        deps.ledger
                            .external_spend(
                                tx,
                                tx_id,
                                ExternalSpendParams {
                                    journal_id: wallet.journal_id,
                                    ledger_account_ids: wallet.ledger_account_ids,
                                    reserved_fees,
                                    meta: ExternalSpendMeta {
                                        encumbered_spending_fee_sats: change_utxo
                                            .as_ref()
                                            .map(|_| fees_to_encumber),
                                        withdraw_from_logical_when_settled: allocations,
                                        tx_summary: TransactionSummary {
                                            wallet_id: wallet.id,
                                            keychain_id,
                                            bitcoin_tx_id: unsynced_tx.tx_id,
                                            total_utxo_in_sats: unsynced_tx.total_utxo_in_sats,
                                            total_utxo_settled_in_sats: settled_sats,
                                            change_sats: change_utxo
                                                .as_ref()
                                                .map(|(utxo, _)| Satoshis::from(utxo.txout.value))
                                                .unwrap_or(Satoshis::ZERO),
                                            fee_sats: unsynced_tx.fee_sats,
                                            change_outpoint: change_utxo
                                                .as_ref()
                                                .map(|(u, _)| u.outpoint),
                                            change_address: change_utxo.map(|(_, a)| a.address),
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
                    .confirm_spend(
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
                        .confirm_spend(
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
                    .confirm_utxo(
                        &mut tx,
                        keychain_id,
                        outpoint,
                        spent,
                        confirmation_time.height,
                    )
                    .await?;
                trackers.n_confirmed_utxos += 1;

                deps.ledger
                    .confirmed_utxo(
                        tx,
                        utxo.confirmed_income_ledger_tx_id,
                        ConfirmedUtxoParams {
                            journal_id: wallet.journal_id,
                            ledger_account_ids: wallet.ledger_account_ids,
                            pending_id: utxo.pending_income_ledger_tx_id,
                            meta: ConfirmedUtxoMeta {
                                wallet_id: data.wallet_id,
                                keychain_id,
                                confirmation_time,
                                satoshis: utxo.value,
                                outpoint,
                                address: utxo.address,
                                already_spent_tx_id: utxo.pending_spend_ledger_tx_id,
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
                    .confirm_spend(
                        &mut tx,
                        keychain_id,
                        inputs.iter().map(|u| &u.outpoint),
                        change_utxo,
                        confirmation_time.height,
                    )
                    .await?
                {
                    deps.ledger
                        .confirm_spend(
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
    }

    span.record("n_pending_utxos", trackers.n_pending_utxos);
    span.record("n_confirmed_utxos", trackers.n_confirmed_utxos);
    span.record("n_found_txs", trackers.n_found_txs);

    Ok(data)
}

async fn fees_for_keychain<T>(keychain: &KeychainWallet<T>) -> Result<Satoshis, BriaError>
where
    T: ToInternalDescriptor + ToExternalDescriptor + Clone + Send + Sync + 'static,
{
    let fee_rate = crate::fee_estimation::MempoolSpaceClient::fee_rate(TxPriority::NextBlock)
        .await?
        .as_sat_per_vb();
    let weight = keychain.max_satisfaction_weight().await?;
    Ok(Satoshis::from((fee_rate as u64) * (weight as u64)))
}

async fn init_electrum(electrum_url: &str) -> Result<(ElectrumBlockchain, u32), BriaError> {
    let blockchain = ElectrumBlockchain::from(Client::from_config(
        electrum_url,
        ConfigBuilder::new()
            .retry(10)
            .timeout(Some(4))
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
