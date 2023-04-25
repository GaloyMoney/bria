mod helpers;

use rand::distributions::{Alphanumeric, DistString};

use bria::{app::*, payout::*, primitives::*, xpub::*};

#[tokio::test]
async fn test_payout() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;

    let original = "tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4";
    let xpub = XPub::try_from((original, Some("m/84'/0'/0'"))).unwrap();
    let wallet_name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let repo = XPubs::new(&pool);

    let id = repo
        .persist(
            NewAccountXPub::builder()
                .account_id(profile.account_id)
                .original(original.to_owned())
                .key_name(wallet_name.clone())
                .value(xpub)
                .build()
                .unwrap(),
        )
        .await?;

    let app = App::run(
        pool,
        true,
        BlockchainConfig::default(),
        WalletsConfig::default(),
    )
    .await?;
    app.create_wallet(profile.clone(), wallet_name.clone(), vec![id.to_string()])
        .await?;

    let group_name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let _ = app
        .create_batch_group(profile.clone(), group_name.clone(), None, None)
        .await?;

    let destination = PayoutDestination::OnchainAddress {
        value: "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap(),
    };
    let _ = app
        .queue_payout(
            profile,
            wallet_name,
            group_name,
            destination,
            Satoshis::from(10000),
            None,
            None,
        )
        .await?;

    Ok(())
}
