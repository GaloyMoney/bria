use derive_builder::Builder;

use super::value::XPub as XPubValue;
use crate::primitives::*;

#[derive(Builder, Clone, Debug)]
pub struct NewXPub {
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) key_name: String,
    pub(super) value: XPubValue,
}

impl NewXPub {
    pub fn builder() -> NewXPubBuilder {
        NewXPubBuilder::default()
    }

    pub fn id(&self) -> XPubId {
        self.value.id()
    }
}
