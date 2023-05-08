use derive_builder::Builder;

use crate::primitives::*;

#[derive(Debug, Clone, Builder)]
pub struct NewDescriptor {
    pub(super) db_uuid: uuid::Uuid,
    pub(super) account_id: AccountId,
    pub(super) wallet_id: WalletId,
    pub(super) descriptor: bitcoin::ExtendedDescriptor,
    pub(super) keychain_kind: bitcoin::KeychainKind,
}

impl NewDescriptor {
    pub fn builder() -> NewDescriptorBuilder {
        let mut builder = NewDescriptorBuilder::default();
        builder.db_uuid(uuid::Uuid::new_v4());
        builder
    }

    pub fn descriptor_and_checksum(&self) -> (String, String) {
        let descriptor = self.descriptor.to_string();
        let checksum = descriptor.split_once('#').unwrap().1.to_string();
        (descriptor, checksum)
    }
}
