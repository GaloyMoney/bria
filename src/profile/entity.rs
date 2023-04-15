use crate::primitives::*;

#[derive(Clone, Debug)]
pub struct Profile {
    pub id: ProfileId,
    pub account_id: AccountId,
    pub name: String,
}

pub struct ProfileApiKey {
    pub key: String,
    pub id: ProfileApiKeyId,
    pub profile_id: ProfileId,
    pub account_id: AccountId,
}
