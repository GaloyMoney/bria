use bdk::blockchain::{ElectrumBlockchain, GetHeight};
use electrum_client::{Client, ConfigBuilder};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    app::BlockchainConfig,
    batch::*,
    bdk::pg::{OldUtxos, PendingUtxo, SettledUtxo, UnsyncedUtxo, Utxos},
    error::*,
    ledger::*,
    primitives::*,
    wallet::*,
    wallet_utxo::WalletUtxos,
};

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
    skip(pool, wallets, batches, wallet_utxos),
    fields(n_pending_utxos, n_settled_utxos),
    err
)]
pub async fn execute(
    pool: sqlx::PgPool,
    wallets: Wallets,
    batches: Batches,
    blockchain_cfg: BlockchainConfig,
    wallet_utxos: WalletUtxos,
    ledger: Ledger,
    data: SyncWalletData,
) -> Result<SyncWalletData, BriaError> {
    let wallet = wallets.find_by_id(data.wallet_id).await?;
    let mut n_pending_utxos = 0;
    let mut n_settled_utxos = 0;
    for keychain_wallet in wallet.keychain_wallets(pool.clone()) {
        let keychain_id = keychain_wallet.keychain_id;
        let blockchain = ElectrumBlockchain::from(
            Client::from_config(
                &blockchain_cfg.electrum_url,
                ConfigBuilder::new().retry(5).build(),
            )
            .unwrap(),
        );
        let current_height = blockchain.get_height()?;
        let _ = keychain_wallet.sync(blockchain).await;
        let utxos = Utxos::new(pool.clone());
        loop {
            let mut tx = pool.begin().await?;
            if let Ok(Some(UnsyncedUtxo {
                local_utxo,
                path,
                confirmation_time,
            })) = utxos.find_unsynced_utxo(&mut tx, keychain_id).await
            {
                let address_info = keychain_wallet
                    .find_address_from_path(path, local_utxo.keychain)
                    .await?;
                if let Some(pending_id) = wallet_utxos
                    .new_bdk_utxo(&mut tx, wallet.id, keychain_id, &address_info, &local_utxo)
                    .await?
                {
                    n_pending_utxos += 1;
                    ledger
                        .incoming_utxo(
                            tx,
                            pending_id,
                            IncomingUtxoParams {
                                journal_id: wallet.journal_id,
                                ledger_account_incoming_id: wallet.pick_dust_or_ledger_account(
                                    &local_utxo,
                                    wallet.ledger_account_ids.incoming_id,
                                ),
                                meta: IncomingUtxoMeta {
                                    wallet_id: data.wallet_id,
                                    keychain_id,
                                    outpoint: local_utxo.outpoint,
                                    satoshis: local_utxo.txout.value.into(),
                                    address: address_info.to_string(),
                                    confirmation_time,
                                },
                            },
                        )
                        .await?;
                }
            } else {
                break;
            }
        }
        // let utxos = OldUtxos::new(keychain_id, pool.clone());
        // loop {
        //     let mut tx = pool.begin().await?;
        //     if let Ok(Some(PendingUtxo {
        //         pending_id,
        //         local_utxo,
        //         confirmation_time,
        //     })) = utxos.find_new_pending_tx(&mut tx).await
        //     {
        //         n_pending_utxos += 1;
        //         ledger
        //             .incoming_utxo(
        //                 tx,
        //                 pending_id,
        //                 IncomingUtxoParams {
        //                     journal_id: wallet.journal_id,
        //                     ledger_account_incoming_id: wallet.pick_dust_or_ledger_account(
        //                         &local_utxo,
        //                         wallet.ledger_account_ids.incoming_id,
        //                     ),
        //                     meta: IncomingUtxoMeta {
        //                         wallet_id: data.wallet_id,
        //                         keychain_id,
        //                         outpoint: local_utxo.outpoint,
        //                         txout: local_utxo.txout,
        //                         confirmation_time,
        //                     },
        //                 },
        //             )
        //             .await?;
        //     } else {
        //         break;
        //     }
        // }

        // let mut utxos_to_skip = Vec::new();
        // loop {
        //     let mut tx = pool.begin().await?;
        //     if let Ok(Some(SettledUtxo {
        //         settled_id,
        //         pending_id,
        //         confirmation_time,
        //         local_utxo,
        //     })) = utxos.find_new_settled_tx(&mut tx, &utxos_to_skip).await
        //     {
        //         n_settled_utxos += 1;
        //         if let Some(batch_id) = batches
        //             .find_containing_utxo(keychain_id, local_utxo.outpoint)
        //             .await?
        //         {
        //             ledger
        //                 .confirmed_utxo_without_fee_reserve(
        //                     tx,
        //                     settled_id,
        //                     ConfirmedUtxoWithoutFeeReserveParams {
        //                         journal_id: wallet.journal_id,
        //                         incoming_ledger_account_id: wallet.pick_dust_or_ledger_account(
        //                             &local_utxo,
        //                             wallet.ledger_account_ids.incoming_id,
        //                         ),
        //                         at_rest_ledger_account_id: wallet.pick_dust_or_ledger_account(
        //                             &local_utxo,
        //                             wallet.ledger_account_ids.at_rest_id,
        //                         ),
        //                         pending_id,
        //                         meta: ConfirmedUtxoWithoutFeeReserveMeta {
        //                             wallet_id: data.wallet_id,
        //                             keychain_id,
        //                             batch_id,
        //                             confirmation_time,
        //                             outpoint: local_utxo.outpoint,
        //                             txout: local_utxo.txout,
        //                         },
        //                     },
        //                 )
        //                 .await?;
        //             continue;
        //         }

        //         if confirmation_time.height
        //             >= current_height - wallet.config.mark_settled_after_n_confs
        //         {
        //             utxos_to_skip.push(local_utxo.outpoint);
        //             continue;
        //         }

        //         let fee_rate =
        //             crate::fee_estimation::MempoolSpaceClient::fee_rate(TxPriority::NextBlock)
        //                 .await?
        //                 .as_sat_per_vb();
        //         let weight = keychain_wallet.max_satisfaction_weight().await?;
        //         let fees = (fee_rate as u64) * (weight as u64);

        //         ledger
        //             .confirmed_utxo(
        //                 tx,
        //                 settled_id,
        //                 ConfirmedUtxoParams {
        //                     journal_id: wallet.journal_id,
        //                     incoming_ledger_account_id: wallet.pick_dust_or_ledger_account(
        //                         &local_utxo,
        //                         wallet.ledger_account_ids.incoming_id,
        //                     ),
        //                     at_rest_ledger_account_id: wallet.pick_dust_or_ledger_account(
        //                         &local_utxo,
        //                         wallet.ledger_account_ids.at_rest_id,
        //                     ),
        //                     fee_ledger_account_id: wallet.ledger_account_ids.fee_id,
        //                     spending_fee_satoshis: match wallet.is_dust_utxo(&local_utxo) {
        //                         true => Satoshis::from(Decimal::ZERO),
        //                         false => Satoshis::from(fees),
        //                     },
        //                     pending_id,
        //                     meta: ConfirmedUtxoMeta {
        //                         wallet_id: data.wallet_id,
        //                         keychain_id,
        //                         confirmation_time,
        //                         outpoint: local_utxo.outpoint,
        //                         txout: local_utxo.txout,
        //                     },
        //                 },
        //             )
        //             .await?;
        //     } else {
        //         break;
        //     }
        // }
    }

    let span = tracing::Span::current();
    span.record("n_pending_utxos", n_pending_utxos);
    span.record("n_settled_utxos", n_settled_utxos);

    Ok(data)
}
