use bdk::blockchain::{ElectrumBlockchain, GetHeight};
use electrum_client::{Client, ConfigBuilder};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    app::BlockchainConfig,
    bdk::pg::{ConfirmedIncomeUtxo, UnsyncedIncomeUtxo, Utxos},
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
    skip(pool, wallets, wallet_utxos),
    fields(n_pending_utxos, n_settled_utxos),
    err
)]
pub async fn execute(
    pool: sqlx::PgPool,
    wallets: Wallets,
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
            if let Ok(Some(UnsyncedIncomeUtxo {
                local_utxo,
                path,
                confirmation_time,
            })) = utxos.find_unsynced_income_utxo(&mut tx, keychain_id).await
            {
                let address_info = keychain_wallet
                    .find_address_from_path(path, local_utxo.keychain)
                    .await?;
                let pending_id = wallet_utxos
                    .new_income_utxo(&mut tx, wallet.id, keychain_id, &address_info, &local_utxo)
                    .await?;
                n_pending_utxos += 1;
                ledger
                    .incoming_utxo(
                        tx,
                        pending_id,
                        OldIncomingUtxoParams {
                            journal_id: wallet.journal_id,
                            ledger_account_incoming_id: wallet.pick_dust_or_ledger_account(
                                local_utxo.txout.value.into(),
                                wallet.ledger_account_ids.onchain_incoming_id,
                            ),
                            meta: OldIncomingUtxoMeta {
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
            } else {
                break;
            }
        }

        loop {
            let mut tx = pool.begin().await?;
            let min_height = current_height - wallet.config.mark_settled_after_n_confs + 1;
            if let Ok(Some(ConfirmedIncomeUtxo {
                outpoint,
                spent,
                confirmation_time,
            })) = utxos
                .find_settled_income_utxo(&mut tx, keychain_id, min_height)
                .await
            {
                let wallet_utxo = wallet_utxos
                    .confirm_income_utxo(
                        &mut tx,
                        keychain_id,
                        outpoint,
                        spent,
                        confirmation_time.height,
                    )
                    .await?;
                n_settled_utxos += 1;

                let fee_rate =
                    crate::fee_estimation::MempoolSpaceClient::fee_rate(TxPriority::NextBlock)
                        .await?
                        .as_sat_per_vb();
                let weight = keychain_wallet.max_satisfaction_weight().await?;
                let fees = (fee_rate as u64) * (weight as u64);

                ledger
                    .confirmed_utxo(
                        tx,
                        wallet_utxo.income_settled_ledger_tx_id,
                        ConfirmedUtxoParams {
                            journal_id: wallet.journal_id,
                            incoming_ledger_account_id: wallet.pick_dust_or_ledger_account(
                                wallet_utxo.value,
                                wallet.ledger_account_ids.onchain_incoming_id,
                            ),
                            at_rest_ledger_account_id: wallet.pick_dust_or_ledger_account(
                                wallet_utxo.value,
                                wallet.ledger_account_ids.onchain_at_rest_id,
                            ),
                            fee_ledger_account_id: wallet.ledger_account_ids.fee_id,
                            spending_fee_satoshis: match wallet.is_dust_utxo(wallet_utxo.value) {
                                true => Satoshis::from(Decimal::ZERO),
                                false => Satoshis::from(fees),
                            },
                            pending_id: wallet_utxo.income_pending_ledger_tx_id,
                            meta: ConfirmedUtxoMeta {
                                wallet_id: data.wallet_id,
                                keychain_id,
                                confirmation_time,
                                satoshis: wallet_utxo.value,
                                outpoint,
                                address: wallet_utxo.address,
                            },
                        },
                    )
                    .await?;
            } else {
                break;
            }
        }
    }

    let span = tracing::Span::current();
    span.record("n_pending_utxos", n_pending_utxos);
    span.record("n_settled_utxos", n_settled_utxos);

    Ok(data)
}
