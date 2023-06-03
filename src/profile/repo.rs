use rand::distributions::{Alphanumeric, DistString};
use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use super::{entity::*, error::ProfileError};
use crate::{dev_constants, error::*, primitives::*};

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
        account_id: AccountId,
        profile_name: String,
    ) -> Result<Profile, BriaError> {
        let id = Uuid::new_v4();
        let record = sqlx::query!(
            r#"INSERT INTO bria_profiles (id, account_id, name)
            VALUES ($1, $2, $3)
            RETURNING (id)"#,
            id,
            Uuid::from(account_id),
            profile_name,
        )
        .fetch_one(&mut *tx)
        .await?;
        Ok(Profile {
            id: ProfileId::from(record.id),
            account_id,
            name: profile_name,
        })
    }

    pub async fn list_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<Profile>, ProfileError> {
        let records = sqlx::query!(
            r#"SELECT id, name FROM bria_profiles WHERE account_id = $1"#,
            account_id as AccountId
        )
        .fetch_all(&self.pool)
        .await?;

        let profiles = records
            .into_iter()
            .map(|record| Profile {
                id: ProfileId::from(record.id),
                account_id,
                name: record.name,
            })
            .collect();

        Ok(profiles)
    }

    pub async fn find_by_name(
        &self,
        account_id: AccountId,
        name: String,
    ) -> Result<Profile, BriaError> {
        let record = sqlx::query!(
            r#"SELECT id, name FROM bria_profiles WHERE account_id = $1 AND name = $2"#,
            Uuid::from(account_id),
            name
        )
        .fetch_optional(&self.pool)
        .await?;

        record
            .map(|row| Profile {
                id: ProfileId::from(row.id),
                account_id,
                name: row.name,
            })
            .ok_or(BriaError::ProfileNotFound)
    }

    pub async fn create_key_for_profile_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, Postgres>,
        profile: Profile,
        dev: bool,
    ) -> Result<ProfileApiKey, BriaError> {
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
        .fetch_one(&mut *tx)
        .await?;
        Ok(ProfileApiKey {
            key,
            id: ProfileApiKeyId::from(record.id),
            profile_id: profile.id,
            account_id: profile.account_id,
        })
    }

    pub async fn find_by_key(&self, key: &str) -> Result<Profile, ProfileError> {
        let record = sqlx::query!(
            r#"SELECT p.id, p.account_id, p.name
               FROM bria_profiles p
               JOIN bria_profile_api_keys k ON k.profile_id = p.id
               WHERE k.active = true AND k.encrypted_key = crypt($1, encrypted_key)"#,
            key
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(record) = record {
            Ok(Profile {
                id: ProfileId::from(record.id),
                account_id: AccountId::from(record.account_id),
                name: record.name,
            })
        } else {
            Err(ProfileError::ProfileNotFoundError)
        }
    }
}
