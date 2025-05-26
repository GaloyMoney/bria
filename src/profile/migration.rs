use es_entity::IntoEvents;
use sqlx::{Pool, Postgres};

use super::{entity::*, error::ProfileError, Profiles};

pub async fn profile_event_migration(pool: &Pool<Postgres>) -> Result<(), ProfileError> {
    let res = sqlx::query!("SELECT count(*) FROM bria_profile_events")
        .fetch_one(pool)
        .await?;

    if res.count.unwrap_or(0) == 0 {
        let records = sqlx::query!(r#"SELECT id, account_id, name FROM bria_profiles"#,)
            .fetch_all(pool)
            .await?;
        let tx = pool.begin().await?;
        let profile = Profiles::new(pool);
        let mut op = profile.begin_op().await?;
        for record in records.into_iter() {
            let new_profile = NewProfile::builder()
                .id(record.id)
                .account_id(record.account_id)
                .name(record.name)
                .build()
                .expect("Failed to build profile");
            profile
                .persist_profile_events(&mut op, &mut new_profile.into_events())
                .await?;
            // profile.persist_events(op, events).await?;
        }
        tx.commit().await?;
    }
    Ok(())
}
