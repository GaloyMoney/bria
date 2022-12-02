mod helpers;

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

    let pending_id = Uuid::new_v4();
    ledger
        .pending_onchain_income(
            tx,
            PendingOnchainIncomeParams {
                journal_id,
                recipient_account_id: ledger_account_id,
                pending_id,
                meta: PendingOnchainIncomeMeta {
                    wallet_id,
                    keychain_id: KeychainId::new(),
                    outpoint,
                    txout,
                },
            },
        )
        .await?;

    let balance = ledger
        .get_balance(journal_id, ledger_account_id)
        .await?
        .expect("No balance");

    assert_eq!(balance.pending(), Decimal::ONE);

    Ok(())
}
