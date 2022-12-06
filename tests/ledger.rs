mod helpers;

use bdk::BlockTime;
use bitcoin::blockdata::transaction::{OutPoint, TxOut};
use bria::{ledger::*, primitives::*};
use rand::distributions::{Alphanumeric, DistString};
use rust_decimal::Decimal;
use uuid::Uuid;

#[tokio::test]
async fn test_ledger() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let ledger = Ledger::init(&pool).await?;

    let account_id = AccountId::new();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let mut tx = pool.begin().await?;
    let journal_id = ledger
        .create_journal_for_account(&mut tx, account_id, name.clone())
        .await?;
    let wallet_id = WalletId::new();
    let ledger_account_id = ledger
        .create_ledger_accounts_for_wallet(&mut tx, wallet_id, &name)
        .await?;

    let satoshis = 100_000_000;
    let outpoint = OutPoint {
        txid: "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap(),
        vout: 0,
    };
    let txout = TxOut {
        value: satoshis,
        script_pubkey: "76a914c0e8c0e8c0e8c0e8c0e8c0e8c0e8c0e8c0e8c0e888ac"
            .parse()
            .unwrap(),
    };

    let keychain_id = KeychainId::new();
    let pending_id = Uuid::new_v4();
    ledger
        .incoming_utxo(
            tx,
            IncomingUtxoParams {
                journal_id,
                recipient_account_id: ledger_account_id,
                pending_id,
                meta: IncomingUtxoMeta {
                    wallet_id,
                    keychain_id,
                    outpoint,
                    txout: txout.clone(),
                    confirmation_time: None,
                },
            },
        )
        .await?;

    let balance = ledger
        .get_balance(journal_id, ledger_account_id)
        .await?
        .expect("No balance");

    assert_eq!(balance.pending(), Decimal::ONE);

    let tx = pool.begin().await?;
    let settled_id = Uuid::new_v4();
    ledger
        .confirmed_utxo(
            tx,
            ConfirmedUtxoParams {
                journal_id,
                recipient_account_id: ledger_account_id,
                pending_id,
                settled_id,
                meta: ConfirmedUtxoMeta {
                    wallet_id,
                    keychain_id,
                    outpoint,
                    txout,
                    confirmation_time: BlockTime {
                        height: 1,
                        timestamp: 123409,
                    },
                },
            },
        )
        .await?;

    let balance = ledger
        .get_balance(journal_id, ledger_account_id)
        .await?
        .expect("No balance");
    assert_eq!(balance.pending(), Decimal::ZERO);
    assert_eq!(balance.settled(), Decimal::ONE);

    Ok(())
}
