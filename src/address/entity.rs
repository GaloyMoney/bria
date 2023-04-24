use crate::primitives::{bitcoin::*, *};
use derive_builder::Builder;

pub struct Address {
    pub id: AddressId,
    pub address: String,
    pub profile_id: ProfileId,
    pub keychain_id: KeychainId,
    pub kind: KeychainKind,
    pub address_idx: u32,
    pub external_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Builder, Clone, Debug)]
pub struct NewAddress {
    pub(super) id: AddressId,
    pub(super) account_id: AccountId,
    pub(super) wallet_id: WalletId,
    #[builder(setter(strip_option))]
    pub(super) profile_id: Option<ProfileId>,
    pub(super) keychain_id: KeychainId,
    #[builder(setter(into))]
    pub(super) address: String,
    pub(super) kind: KeychainKind,
    pub(super) address_idx: u32,
    #[builder(setter(strip_option, into))]
    pub(super) external_id: String,
    pub(super) metadata: Option<serde_json::Value>,
}

impl NewAddress {
    pub fn builder() -> NewAddressBuilder {
        let mut builder = NewAddressBuilder::default();
        let new_address_id = AddressId::new();
        builder.external_id(new_address_id.to_string());
        builder.id(new_address_id);
        builder
    }
}
