use std::fmt::{self, Display};

use crate::primitives::{bitcoin::*, *};
use derive_builder::Builder;

#[derive(Clone, Debug, PartialEq)]
pub struct AddressCreationInfo {
    address_string: String,
    address_idx: u32,
    kind: pg::PgKeychainKind,
}

impl AddressCreationInfo {
    pub fn new(address_string: String, address_idx: u32, kind: pg::PgKeychainKind) -> Self {
        AddressCreationInfo {
            address_string,
            address_idx,
            kind,
        }
    }
}

impl Display for AddressCreationInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.address_string)
    }
}

impl From<bdk::wallet::AddressInfo> for AddressCreationInfo {
    fn from(info: bdk::wallet::AddressInfo) -> Self {
        let address_string = info.address.to_string();
        let address_idx = info.index;
        let kind = match info.keychain {
            KeychainKind::External => pg::PgKeychainKind::External,
            KeychainKind::Internal => pg::PgKeychainKind::Internal,
        };

        AddressCreationInfo::new(address_string, address_idx, kind)
    }
}

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

impl NewAddressBuilder {
    pub fn from_address_creation_info(&mut self, info: AddressCreationInfo) -> &mut Self {
        self.address_string(info.address_string);
        self.kind(info.kind);
        self.address_idx(info.address_idx);
        self
    }
}
