mod helpers;

use rand::distributions::{Alphanumeric, DistString};

use bria::{app::*, xpub::*};
use helpers::*;

#[tokio::test]
async fn test_wallet() -> anyhow::Result<()> {
    let pg_host = std::env::var("PG_HOST").unwrap_or("localhost".to_string());
    let pg_con = format!("postgres://user:password@{pg_host}:5432/pg");
    let pool = sqlx::PgPool::connect(&pg_con).await?;

    let account_id = create_test_account(&pool).await?;
    let xpub = XPub::try_from(("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4", Some("m/84'/0'/0'"))).unwrap();
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let repo = XPubs::new(&pool);
    let id = repo.persist(account_id, name.clone(), xpub).await?;

    let app = App::new(pool);
    app.create_wallet(account_id, name.clone(), vec![id.to_string()])
        .await?;
    // app.gen_address(account_id, name).await?;

    Ok(())
}
