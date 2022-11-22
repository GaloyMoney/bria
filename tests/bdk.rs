mod helpers;

use bitcoin::Network;
use uuid::Uuid;

use bria::bdk::*;

#[tokio::test]
async fn test_get_address() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let xpub = "tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4";
    let descriptor = format!("wpkh([8df69d29/84'/0'/0']{}/0/*)", xpub);
    let wallet = BdkWallet::new(pool, Network::Regtest, Uuid::new_v4().into(), descriptor);

    let addr = wallet.next_address().await?;
    println!("addr: {:?}", addr);
    let addr = wallet.next_address().await?;
    println!("addr: {:?}", addr);
    // assert_eq!(format!("{:?}", addr), "543");
    Ok(())
}
