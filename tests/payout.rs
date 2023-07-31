mod helpers;

use rand::distributions::{Alphanumeric, DistString};

use bria::{app::*, primitives::*, xpub::*};

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

    let app = App::run(pool, AppConfig::default()).await?;
    app.create_wpkh_wallet(profile.clone(), wallet_name.clone(), id.to_string(), None)
        .await?;

    let group_name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let _ = app
        .create_payout_queue(profile.clone(), group_name.clone(), None, None)
        .await?;

    let address = "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap();
    let _ = app
        .submit_payout_to_address(
            profile,
            wallet_name,
            group_name,
            address,
            Satoshis::from(10000),
            None,
            None,
        )
        .await?;

    Ok(())
}
