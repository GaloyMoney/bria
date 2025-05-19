use sqlx_ledger::JournalId;
use es_entity::*;

use crate::primitives::*;

#[derive(Debug)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
}

impl Account {
    pub fn journal_id(&self) -> JournalId {
        JournalId::from(self.id)
    }
}
