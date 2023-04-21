mod helpers;

use rand::distributions::{Alphanumeric, DistString};
use serde_json::json;

use bria::{app::*, xpub::*};

#[tokio::test]
async fn test_wallet() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;

    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84'/0'/0'"))).unwrap();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let external_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let metadata = json!({ "foo": "bar" });
    let repo = XPubs::new(&pool);

    let id = repo
        .persist(
            NewXPub::builder()
                .account_id(profile.account_id)
                .key_name(name.clone())
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
    app.create_wallet(profile.clone(), name.clone(), vec![id.to_string()])
        .await?;

    let addr = app
        .new_address(profile.clone(), name.clone(), None, None)
        .await?;
    assert_eq!(addr, "bcrt1qzg4a08kc2xrp08d9k5jadm78ehf7catp735zn0");
    let addr = app
        .new_address(profile, name, Some(external_id), Some(metadata))
        .await?;
    assert_eq!(addr, "bcrt1q6q79yce8vutqzpnwkxr5x8p5kxw5rc0hqqzwym");

    Ok(())
}
