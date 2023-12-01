mod helpers;

use rand::distributions::{Alphanumeric, DistString};

use bria::{
    app::{error::ApplicationError, *},
    primitives::*,
    profile::SpendingPolicy,
    xpub::*,
};

#[tokio::test]
async fn payout() -> anyhow::Result<()> {
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
    app.create_wpkh_wallet(&profile, wallet_name.clone(), id.to_string(), None)
        .await?;

    let group_name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let _ = app
        .create_payout_queue(&profile, group_name.clone(), None, None)
        .await?;

    let address = "mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU".parse().unwrap();
    let _ = app
        .submit_payout_to_address(
            &profile,
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

#[tokio::test]
async fn spending_policy() -> anyhow::Result<()> {
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
    app.create_wpkh_wallet(&profile, wallet_name.clone(), id.to_string(), None)
        .await?;

    let queue_name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let _ = app
        .create_payout_queue(&profile, queue_name.clone(), None, None)
        .await?;
    let address = Address::parse_from_trusted_source("mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU");
    let spending_profile = app
        .create_profile(
            &profile,
            wallet_name.clone(),
            Some(SpendingPolicy {
                allowed_payout_addresses: vec![address.clone()],
            }),
        )
        .await?;

    let _ = app
        .submit_payout_to_address(
            &spending_profile,
            wallet_name.clone(),
            queue_name.clone(),
            address.to_string(),
            Satoshis::from(10000),
            None,
            None,
        )
        .await?;

    let address = "n4VQ5YdHf7hLQ2gWQYYrcxoE5B7nWuDFNF".parse().unwrap();

    let res = app
        .submit_payout_to_address(
            &spending_profile,
            wallet_name,
            queue_name,
            address,
            Satoshis::from(10000),
            None,
            None,
        )
        .await;
    assert!(matches!(
        res,
        Err(ApplicationError::DestinationNotAllowed(_))
    ));

    Ok(())
}
