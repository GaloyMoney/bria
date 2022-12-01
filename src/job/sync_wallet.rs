use bdk::blockchain::ElectrumBlockchain;
use electrum_client::Client;

use crate::{app::BlockchainConfig, bdk::pg::Utxos, error::*, primitives::*, wallet::*};

pub async fn execute(
    pool: sqlx::PgPool,
    wallets: Wallets,
    id: WalletId,
    blockchain_cfg: BlockchainConfig,
) -> Result<(), BriaError> {
    let wallet = wallets.find_by_id(id).await?;
    // let ledger = sqlx_ledger::SqlxLedger::new(&pool);
    // let new_utxo_tx_id = ledger.tx_templates().create()
    for (keychain_id, cfg) in wallet.keychains {
        let keychain_wallet =
            KeychainWallet::new(pool.clone(), blockchain_cfg.network, keychain_id, cfg);
        let blockchain =
            ElectrumBlockchain::from(Client::new(&blockchain_cfg.electrum_url).unwrap());
        let _ = keychain_wallet.sync(blockchain).await;
        let utxos = Utxos::new(keychain_id, pool.clone());
        let mut tx = pool.begin().await?;
        if let Ok(new_pending_tx) = utxos.list_without_pending_tx(&mut tx).await {
            //
        }
    }
    Ok(())
}
