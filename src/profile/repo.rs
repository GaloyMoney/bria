use rand::distributions::{Alphanumeric, DistString};
use sqlx::{Pool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

use super::{entity::*, error::ProfileError};
use crate::{dev_constants, entity::*, primitives::*};

pub struct Profiles {
    pool: Pool<Postgres>,
}

impl Profiles {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create_in_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        profile: NewProfile,
    ) -> Result<Profile, ProfileError> {
        let id = profile.id;
        sqlx::query!(
            r#"INSERT INTO bria_profiles (id, account_id, name)
            VALUES ($1, $2, $3)"#,
            profile.id as ProfileId,
            profile.account_id as AccountId,
            profile.name,
        )
        .execute(&mut **tx)
        .await?;
        let events = profile.initial_events();
        EntityEvents::<ProfileEvent>::persist(
            "bria_profile_events",
            &mut *tx,
            events.new_serialized_events(id),
        )
        .await?;
        let res = Profile::try_from(events)?;
        Ok(res)
    }

    pub async fn list_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Profile>, ProfileError> {
        let rows = sqlx::query!(
            r#"SELECT p.id, e.sequence, e.event_type, e.event
               FROM bria_profiles p
               JOIN bria_profile_events e ON p.id = e.id
               WHERE p.account_id = $1
               ORDER BY p.id, sequence"#,
            account_id as AccountId
        )
        .fetch_all(&self.pool)
        .await?;
        let mut entity_events = HashMap::new();
        for row in rows {
            let id = SigningSessionId::from(row.id);
            let events = entity_events.entry(id).or_insert_with(EntityEvents::new);
            events.load_event(row.sequence as usize, row.event)?;
        }
        let mut profiles = Vec::new();
        for (_, events) in entity_events {
            let profile = Profile::try_from(events)?;
            profiles.push(profile);
        }

        Ok(profiles)
    }

    pub async fn find_by_id(
        &self,
        account_id: AccountId,
        id: ProfileId,
    ) -> Result<Profile, ProfileError> {
        let rows = sqlx::query!(
            r#"SELECT p.id, e.sequence, e.event_type, e.event
               FROM bria_profiles p
               JOIN bria_profile_events e ON p.id = e.id
               WHERE p.account_id = $1 AND p.id = $2
               ORDER BY p.id, sequence"#,
            account_id as AccountId,
            id as ProfileId
        )
        .fetch_all(&self.pool)
        .await?;

        if !rows.is_empty() {
            let mut events = EntityEvents::new();
            for row in rows {
                events.load_event(row.sequence as usize, row.event)?;
            }
            Ok(Profile::try_from(events)?)
        } else {
            Err(ProfileError::ProfileIdNotFound(id))
        }
    }

    pub async fn find_by_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<Profile, ProfileError> {
        let rows = sqlx::query!(
            r#"SELECT p.id, e.sequence, e.event_type, e.event
               FROM bria_profiles p
               JOIN bria_profile_events e ON p.id = e.id
               WHERE p.account_id = $1 AND p.name = $2
               ORDER BY p.id, sequence"#,
            account_id as AccountId,
            name
        )
        .fetch_all(&self.pool)
        .await?;

        if !rows.is_empty() {
            let mut events = EntityEvents::new();
            for row in rows {
                events.load_event(row.sequence as usize, row.event)?;
            }
            Ok(Profile::try_from(events)?)
        } else {
            Err(ProfileError::ProfileNameNotFound(name))
        }
    }

    pub async fn create_key_for_profile_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        profile: Profile,
        dev: bool,
    ) -> Result<ProfileApiKey, ProfileError> {
        let key = if dev {
            dev_constants::BRIA_DEV_KEY.to_string()
        } else {
            let code = Alphanumeric.sample_string(&mut rand::thread_rng(), 64);
            format!("bria_{code}")
        };
        let record = sqlx::query!(
            r#"INSERT INTO bria_profile_api_keys (encrypted_key, profile_id)
            VALUES (crypt($1, gen_salt('bf')), (SELECT id FROM bria_profiles WHERE id = $2)) RETURNING (id)"#,
            key,
            Uuid::from(profile.id),
        )
            .fetch_one(&mut **tx)
            .await?;
        Ok(ProfileApiKey {
            key,
            id: ProfileApiKeyId::from(record.id),
            profile_id: profile.id,
            account_id: profile.account_id,
        })
    }

    pub async fn find_by_key(&self, key: &str) -> Result<Profile, ProfileError> {
        let mut tx = self.pool.begin().await?;

        let record = sqlx::query!(
            r#"SELECT p.id, p.account_id, p.name
               FROM bria_profiles p
               JOIN bria_profile_api_keys k ON k.profile_id = p.id
               WHERE k.active = true AND k.encrypted_key = crypt($1, encrypted_key)"#,
            key
        )
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(record) = record {
            let rows = sqlx::query!(
                r#"SELECT sequence, event_type, event FROM bria_profile_events
               WHERE id = $1
               ORDER BY sequence"#,
                record.id
            )
            .fetch_all(&mut *tx)
            .await?;
            let mut events = EntityEvents::new();
            for row in rows {
                events.load_event(row.sequence as usize, row.event)?;
            }
            Ok(Profile::try_from(events)?)
        } else {
            Err(ProfileError::ProfileKeyNotFound)
        }
    }

    pub async fn update(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        profile: Profile,
    ) -> Result<(), ProfileError> {
        if !profile.events.is_dirty() {
            return Ok(());
        }
        EntityEvents::<ProfileEvent>::persist(
            "bria_profile_events",
            tx,
            profile.events.new_serialized_events(profile.id),
        )
        .await?;
        Ok(())
    }
}
