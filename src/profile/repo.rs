use es_entity::*;
use rand::distributions::{Alphanumeric, DistString};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

use super::{entity::*, error::ProfileError};
use crate::{dev_constants, primitives::*};

#[derive(EsRepo)]
#[es_repo(
    entity = "Profile",
    err = "ProfileError",
    columns(
        name(ty = "String"),
        account_id(ty = "AccountId", list_for, update(persist = false))
    ),
    tbl_prefix = "bria"
)]
pub struct Profiles {
    pool: Pool<Postgres>,
}

impl Profiles {
    pub fn new(pool: &Pool<Postgres>) -> Self {
        Self { pool: pool.clone() }
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

    pub async fn find_by_account_id_and_id(
        &self,
        account_id: AccountId,
        id: ProfileId,
    ) -> Result<Profile, ProfileError> {
        let profile = self.find_by_id(id).await?;
        if profile.account_id != account_id {
            return Err(ProfileError::EsEntityError(EsEntityError::NotFound));
        }
        Ok(profile)
    }

    pub async fn find_by_account_id_and_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<Profile, ProfileError> {
        let profile = es_entity::es_query!(
            "bria",
            &self.pool,
            r#"
            SELECT *
            FROM bria_profiles
            WHERE account_id = $1 and name = $2"#,
            account_id as AccountId,
            name
        )
        .fetch_one()
        .await?;
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
            let profile = self.find_by_id(ProfileId::from(record.id)).await;
            profile
        } else {
            Err(ProfileError::ProfileKeyNotFound)
        }
    }
}
