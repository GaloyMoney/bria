use std::thread::sleep;

use es_entity::*;
use rand::distributions::{Alphanumeric, DistString};
use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use super::{entity::*, error::ProfileError};
use crate::{dev_constants, primitives::*};

#[derive(EsRepo, Clone, Debug)]
#[es_repo(
    entity = "Profile",
    err = "ProfileError",
    columns(name(ty = "String"), account_id(ty = "AccountId", list_for)),
    tbl_prefix = "bria"
)]
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
        let events = profile.into_events();
        EntityEvents::<ProfileEvent>::persist(
            "bria_profile_events",
            &mut *tx,
            events.new_serialized_events(id),
        )
        .await?;
        let res = Profile::try_from_events(events)?;
        Ok(res)
    }

    pub async fn list_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Profile>, ProfileError> {
        let mut profiles = Vec::new();
        let mut next = Some(PaginatedQueryArgs::default());

        while let Some(query) = next.take() {
            let mut res = self
                .list_for_account_id_by_id(account_id, query, Default::default())
                .await?;

            profiles.append(&mut res.entities);
            next = res.into_next_query();
        }

        Ok(profiles)
    }

    pub async fn find_by_id_and_account_id(
        &self,
        id: ProfileId,
        account_id: AccountId,
    ) -> Result<Profile, ProfileError> {
        let profile = self.find_by_id(id).await?;

        if profile.account_id != account_id {
            return Err(ProfileError::EsEntityError(EsEntityError::NotFound));
        }
        Ok(profile)
    }

    pub async fn find_by_name_and_account_id(
        &self,
        name: String,
        account_id: AccountId,
    ) -> Result<Profile, ProfileError> {
        let profile = self.find_by_name(name).await?;

        if profile.account_id != account_id {
            return Err(ProfileError::EsEntityError(EsEntityError::NotFound));
        }
        Ok(profile)
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
            // let rows = sqlx::query!(
            //     r#"SELECT sequence, event_type, event FROM bria_profile_events
            //    WHERE id = $1
            //    ORDER BY sequence"#,
            //     record.id
            // )
            // .fetch_all(&mut *tx)
            // .await?;
            // let mut events = EntityEvents::new();
            // for row in rows {
            //     events.load_event(row.sequence as usize, row.event)?;
            // }
            // Ok(Profile::try_from(events)?)
            let profile = self.find_by_id(ProfileId::from(record.id)).await;
            profile
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
