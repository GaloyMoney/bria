use crate::{account::keys::*, error::*, primitives::*, xpub::*};

pub struct App {
    keys: AccountApiKeys,
    xpubs: XPubs,
}

impl App {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            keys: AccountApiKeys::new(&pool),
            xpubs: XPubs::new(&pool),
        }
    }

    pub async fn authenticate(&self, key: &str) -> Result<AccountId, BriaError> {
        let key = self.keys.find_by_key(key).await?;
        Ok(key.account_id)
    }

    pub async fn import_xpub(
        &self,
        account_id: AccountId,
        name: String,
        xpub: String,
    ) -> Result<XPubId, BriaError> {
        let id = self.xpubs.persist(account_id, name, xpub).await?;
        Ok(id)
    }
}
