mod helpers;

use uuid::Uuid;

use bria::bdk::*;

#[tokio::test]
async fn test_get_address() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let xpub = "tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4";
    let descriptor = format!("wpkh({})", xpub);
    let wallet = BdkWallet::new(pool, Uuid::new_v4().into(), descriptor);

    wallet.next_address().await?;
    Ok(())
}
