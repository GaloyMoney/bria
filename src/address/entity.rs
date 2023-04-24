use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::{
    entity::*,
    primitives::{bitcoin::*, *},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AddressEvent {
    AddressInitialized,
    AddressExternalIdUpdated { external_id: String },
    AddressMetadataUpdated { metadata: serde_json::Value },
}

pub struct WalletAddress {
    pub address: bitcoin::Address,
    pub wallet_id: WalletId,
    pub(super) events: EntityEvents<AddressEvent>,
}

impl WalletAddress {
    pub fn external_id(&self) -> String {
        let mut ret = None;
        for event in self.events.iter() {
            match event {
                AddressEvent::AddressExternalIdUpdated { external_id } => {
                    ret = Some(external_id.clone())
                }
                _ => {}
            }
        }
        ret.expect("external_id not set")
    }

    pub fn metadata(&self) -> Option<&serde_json::Value> {
        let mut ret = None;
        for event in self.events.iter() {
            match event {
                AddressEvent::AddressMetadataUpdated { metadata } => ret = Some(metadata),
                _ => {}
            }
        }
        ret
    }
}

#[derive(Builder, Clone, Debug)]
pub struct NewAddress {
    pub(super) id: AddressId,
    pub(super) address: bitcoin::Address,
    #[builder(setter(into))]
    pub(super) address_idx: u32,
    pub(super) account_id: AccountId,
    pub(super) wallet_id: WalletId,
    #[builder(setter(strip_option))]
    pub(super) profile_id: Option<ProfileId>,
    pub(super) keychain_id: KeychainId,
    #[builder(setter(into))]
    pub(super) external_id: String,
    pub(super) kind: KeychainKind,
    metadata: Option<serde_json::Value>,
}

impl NewAddress {
    pub fn builder() -> NewAddressBuilder {
        let mut builder = NewAddressBuilder::default();
        let new_address_id = AddressId::new();
        builder.external_id(new_address_id.to_string());
        builder.id(new_address_id);
        builder
    }

    pub fn initial_events(self) -> EntityEvents<AddressEvent> {
        let mut events = EntityEvents::init([
            AddressEvent::AddressInitialized,
            AddressEvent::AddressExternalIdUpdated {
                external_id: self.external_id.clone(),
            },
        ]);
        if let Some(metadata) = self.metadata {
            events.push(AddressEvent::AddressMetadataUpdated { metadata })
        }
        events
    }
}
