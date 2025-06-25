mod helpers;

use rand::distributions::{Alphanumeric, DistString};
use serde_json::json;

use bria::{app::*, xpub::*};

#[tokio::test]
async fn create_wpkh_wallet() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;

    let original = "tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4";
    let xpub = XPub::try_from((original, Some("m/84'/0'/0'"))).unwrap();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let external_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let repo = XPubs::new(&pool);

    let id = repo
        .create(
            NewAccountXPub::builder()
                .account_id(profile.account_id)
                .original(original.to_owned())
                .key_name(name.clone())
                .value(xpub)
                .build()
                .unwrap(),
        )
        .await?
        .fingerprint();
    let app = App::run(pool, AppConfig::default()).await?;
    app.create_wpkh_wallet(&profile, name.clone(), id.to_string(), None)
        .await?;

    let (_, addr) = app.new_address(&profile, name.clone(), None, None).await?;
    assert_eq!(
        addr.to_string(),
        "bcrt1qzg4a08kc2xrp08d9k5jadm78ehf7catp735zn0"
    );
    let metadata = json!({ "foo": "bar" });
    let (_, addr) = app
        .new_address(&profile, name, Some(external_id), Some(metadata))
        .await?;
    assert_eq!(
        addr.to_string(),
        "bcrt1q6q79yce8vutqzpnwkxr5x8p5kxw5rc0hqqzwym"
    );

    Ok(())
}

#[tokio::test]
async fn create_descriptors_wallet() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;

    let external = "wpkh([1ff51810/84'/0'/0']tpubDDdzmt7vndmNywiVAeBPuhYLTFa7hmtfaqUxxTv5iLy7bxU93B62M9WKFSmn1BEN2vte8GDD3SUNKbupRajFW4RK8hd3i6W15pvTRQfo1fK/0/*)#q8r69l4d".to_owned();
    let internal = "wpkh([1ff51810/84'/0'/0']tpubDDdzmt7vndmNywiVAeBPuhYLTFa7hmtfaqUxxTv5iLy7bxU93B62M9WKFSmn1BEN2vte8GDD3SUNKbupRajFW4RK8hd3i6W15pvTRQfo1fK/1/*)#3nxmc294".to_owned();
    let app = App::run(pool, AppConfig::default()).await?;
    let wallet_name = "test_import_descriptor".to_owned();
    let (_, xpub_fingerprints) = app
        .create_descriptors_wallet(&profile, wallet_name, external, internal)
        .await?;
    assert_eq!(xpub_fingerprints.len(), 1);
    assert_eq!(xpub_fingerprints[0].to_string(), "2f18f2f7");
    Ok(())
}
