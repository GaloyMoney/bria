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
    let repo = XPubs::new(&pool);
    let id = repo
        .persist(account_id, "wallet".to_string(), "wallet".to_string())
        .await?;
    let app = App::new(pool);
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    app.create_wallet(account_id, name, vec![id.to_string()])
        .await?;

    Ok(())
}
