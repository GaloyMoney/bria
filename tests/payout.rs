mod helpers;

use rand::distributions::{Alphanumeric, DistString};

use bria::{app::*, payout::*, xpub::*};

#[tokio::test]
async fn test_payout() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let account_id = helpers::create_test_account(&pool).await?;

    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84'/0'/0'"))).unwrap();
    let wallet_name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let repo = XPubs::new(&pool);

    let id = repo.persist(account_id, wallet_name.clone(), xpub).await?;

    let app = App::run(pool, BlockchainConfig::default(), WalletsConfig::default()).await?;
    app.create_wallet(account_id, wallet_name.clone(), vec![id.to_string()])
        .await?;

    let group_name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let _ = app
        .create_batch_group(account_id, group_name.clone())
        .await?;

    let destination = PayoutDestination::OnchainAddress {
        value: "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap(),
    };
    let _ = app
        .queue_payout(
            account_id,
            wallet_name,
            group_name,
            destination,
            10000,
            None,
            None,
        )
        .await?;

    Ok(())
}
