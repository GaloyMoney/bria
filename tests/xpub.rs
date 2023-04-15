mod helpers;

use bria::xpub::*;

#[tokio::test]
async fn test_xpub() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let profile = helpers::create_test_account(&pool).await?;

    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84'/0'/0'"))).unwrap();

    let repo = XPubs::new(&pool);
    let _ = repo
        .persist(
            NewXPub::builder()
                .account_id(profile.account_id)
                .key_name("name")
                .value(xpub)
                .build()
                .unwrap(),
        )
        .await?;

    Ok(())
}
