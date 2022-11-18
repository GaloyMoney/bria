use bria::xpub::*;
// use helpers::*;

#[tokio::test]
async fn test_xpub() -> anyhow::Result<()> {
    let pg_host = std::env::var("PG_HOST").unwrap_or("localhost".to_string());
    let pg_con = format!("postgres://bria:bria@{pg_host}:5432/bria");
    let pool = sqlx::PgPool::connect(&pg_con).await?;

    // let account_id = create_test_account(&pool).await?;

    let repo = XPubs::new(&pool);
    // repo.persist(

    Ok(())
}
