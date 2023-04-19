use crate::primitives::{bitcoin::*, *};
use derive_builder::Builder;

pub struct Address {
    pub id: AddressId,
    pub address_string: String,
    pub keychain_id: KeychainId,
    pub kind: pg::PgKeychainKind,
    pub address_idx: u32,
    pub external_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Builder, Clone, Debug)]
pub struct NewAddress {
    pub id: AddressId,
    #[builder(setter(into))]
    pub address_string: String,
    pub keychain_id: KeychainId,
    pub kind: pg::PgKeychainKind,
    pub address_idx: u32,
    #[builder(setter(into))]
    pub external_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl NewAddress {
    pub fn builder() -> NewAddressBuilder {
        let mut builder = NewAddressBuilder::default();
        builder.id(AddressId::new());
        builder
    }
}
