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
    utxo::{Utxos, WalletUtxo},
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
    bria_utxos: Utxos,
    ledger: Ledger,
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
    let mut trackers = InstrumentationTrackers::new();
    let deps = Deps {
        blockchain_cfg,
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
                            if wallet.config.min_settle_height(current_height, spend_tx)
                                <= t.height =>
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
                                    },
                                },
                            )
                            .await?;
                    }
                }
            }
            if spend_tx {
                let change_utxo = if !change.is_empty() {
                    let (utxo, path) = change.remove(0);
                    let address_info = keychain_wallet
                        .find_address_from_path(path, utxo.keychain)
                        .await?;
                    Some((utxo, address_info))
                } else {
                    None
                };
                if let Some((tx_id, tx)) = deps
                    .bria_utxos
                    .mark_spent(
                        wallet.id,
                        keychain_id,
                        income_bria_utxos
                            .iter()
                            .map(|WalletUtxo { outpoint, .. }| outpoint),
                        change_utxo.as_ref(),
                        unsynced_tx.sats_per_vbyte_when_created,
                    )
                    .await?
                {
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
                                total_utxo_in_sats: unsynced_tx.total_utxo_in_sats,
                                total_utxo_settled_in_sats: unsynced_tx.total_utxo_in_sats,
                                change_sats: change_utxo
                                    .as_ref()
                                    .map(|(utxo, _)| Satoshis::from(utxo.txout.value))
                                    .unwrap_or(Satoshis::ZERO),
                                fee_sats: unsynced_tx.fee_sats,
                                reserved_fees,
                                meta: ExternalSpendMeta {
                                    wallet_id: wallet.id,
                                    keychain_id,
                                    encumbered_spending_fee_sats: change_utxo
                                        .as_ref()
                                        .map(|_| fees_to_encumber),
                                    change_outpoint: change_utxo.as_ref().map(|(u, _)| u.outpoint),
                                    change_address: change_utxo.map(|(_, a)| a.address),
                                    confirmation_time: unsynced_tx.confirmation_time,
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
            let min_height = current_height - wallet.config.settle_income_after_n_confs + 1;
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
                            },
                        },
                    )
                    .await?;
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
    let blockchain = ElectrumBlockchain::from(
        Client::from_config(
            electrum_url,
            ConfigBuilder::new()
                .retry(10)
                .timeout(Some(4))
                .expect("couldn't set electrum timeout")
                .build(),
        )
        .unwrap(),
    );
    let current_height = blockchain.get_height()?;
    Ok((blockchain, current_height))
}
