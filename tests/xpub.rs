mod helpers;

use bria::xpub::*;

#[tokio::test]
async fn test_xpub() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let account_id = helpers::create_test_account(&pool).await?;

    let xpub: XPub = "tpubDD6sGNgWVAeKaMGF5XkfBhMAuSqjoiqUoSM7Dmf11auxu41PDg1AL4LDwTkuVEMUS2zY51zPESy1xr26cLj7BZHfwZQHd4Xf1Ym5WbvAMru".parse()?;

    let repo = XPubs::new(&pool);
    repo.persist(account_id, "name".to_string(), xpub).await?;

    Ok(())
}
