use bdk::blockchain::{ElectrumBlockchain, GetHeight};
use electrum_client::Client;
use tracing::instrument;

use crate::{
    app::BlockchainConfig,
    bdk::pg::{NewPendingTx, NewSettledTx, Utxos},
    error::*,
    ledger::*,
    primitives::*,
    wallet::*,
};

#[instrument(name = "job.sync_wallet", skip(pool, wallets, ledger), err)]
pub async fn execute(
    pool: sqlx::PgPool,
    wallets: Wallets,
    id: WalletId,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
) -> Result<(), BriaError> {
    let wallet = wallets.find_by_id(id).await?;
    for (keychain_id, cfg) in wallet.keychains.iter() {
        let keychain_wallet = KeychainWallet::new(
            pool.clone(),
            blockchain_cfg.network,
            *keychain_id,
            cfg.clone(),
        );
        let blockchain =
            ElectrumBlockchain::from(Client::new(&blockchain_cfg.electrum_url).unwrap());
        let current_height = blockchain.get_height()?;
        let _ = keychain_wallet.sync(blockchain).await;
        let utxos = Utxos::new(*keychain_id, pool.clone());
        loop {
            let mut tx = pool.begin().await?;
            if let Ok(Some(NewPendingTx {
                pending_id,
                local_utxo,
                confirmation_time,
            })) = utxos.find_new_pending_tx(&mut tx).await
            {
                ledger
                    .incoming_utxo(
                        tx,
                        IncomingUtxoParams {
                            journal_id: wallet.journal_id,
                            ledger_account_incoming_id: wallet.pick_dust_or_ledger_account(
                                &local_utxo,
                                wallet.ledger_account_ids.incoming_id,
                            ),
                            pending_id,
                            meta: IncomingUtxoMeta {
                                wallet_id: id,
                                keychain_id: *keychain_id,
                                outpoint: local_utxo.outpoint,
                                txout: local_utxo.txout,
                                confirmation_time,
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
            if let Ok(Some(NewSettledTx {
                settled_id,
                pending_id,
                confirmation_time,
                local_utxo,
            })) = utxos
                .find_new_settled_tx(
                    &mut tx,
                    current_height - wallet.config.mark_settled_after_n_confs + 1,
                )
                .await
            {
                let fee_rate =
                    crate::fee_estimation::MempoolSpaceClient::fee_rate(TxPriority::NextBlock)
                        .await?;

                ledger
                    .confirmed_utxo(
                        tx,
                        ConfirmedUtxoParams {
                            journal_id: wallet.journal_id,
                            ledger_account_incoming_id: wallet.pick_dust_or_ledger_account(
                                &local_utxo,
                                wallet.ledger_account_ids.incoming_id,
                            ),
                            ledger_account_at_rest_id: wallet.pick_dust_or_ledger_account(
                                &local_utxo,
                                wallet.ledger_account_ids.at_rest_id,
                            ),
                            ledger_account_fee_id: wallet.ledger_account_ids.fee_id,
                            fees: (fee_rate.as_sat_per_vb() as i64).into(), // WIP
                            pending_id,
                            settled_id,
                            meta: ConfirmedUtxoMeta {
                                wallet_id: id,
                                keychain_id: *keychain_id,
                                confirmation_time,
                                outpoint: local_utxo.outpoint,
                                txout: local_utxo.txout,
                            },
                        },
                    )
                    .await?;
            } else {
                break;
            }
        }
    }
    Ok(())
}
