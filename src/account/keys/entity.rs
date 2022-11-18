use crate::primitives::*;

pub struct AccountApiKey {
    pub name: String,
    pub key: String,
    pub id: AccountApiKeyId,
    pub account_id: AccountId,
}
