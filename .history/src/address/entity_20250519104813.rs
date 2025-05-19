use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use es_entity::*;
use crate::{
    primitives::{bitcoin::*, *},
};

#[derive(EsEvent, Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[es_event(id = "PayoutQueueId")]

#[serde(tag = "type", rename_all = "snake_case")]
pub enum AddressEvent {
    Initialized {
        db_uuid: uuid::Uuid,
        account_id: AccountId,
        wallet_id: WalletId,
        keychain_id: KeychainId,
        profile_id: Option<ProfileId>,
        address: Address,
        address_idx: u32,
        kind: KeychainKind,
    },
    ExternalIdUpdated {
        external_id: String,
    },
    MetadataUpdated {
        metadata: serde_json::Value,
    },
}

#[derive(Debug, Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct WalletAddress {
    pub account_id: AccountId,
    pub address: Address,
    pub wallet_id: WalletId,
    pub external_id: String,
    kind: KeychainKind,
    pub(super) db_uuid: uuid::Uuid,
    pub(super) events: EntityEvents<AddressEvent>,
}

impl WalletAddress {
    pub fn metadata(&self) -> Option<&serde_json::Value> {
        let mut ret = None;
        for event in self.events.iter() {
            if let AddressEvent::MetadataUpdated { metadata } = event {
                ret = Some(metadata)
            }
        }
        ret
    }

    pub fn update_external_id(&mut self, external_id: String) {
        if self.external_id != external_id {
            self.external_id.clone_from(&external_id);
            self.events
                .push(AddressEvent::ExternalIdUpdated { external_id });
        }
    }

    pub fn update_metadata(&mut self, metadata: serde_json::Value) {
        if self.metadata() != Some(&metadata) {
            self.events.push(AddressEvent::MetadataUpdated { metadata });
        }
    }

    pub fn is_external(&self) -> bool {
        self.kind == KeychainKind::External
    }
}

#[derive(Builder, Clone, Debug)]
pub struct NewAddress {
    pub(super) db_uuid: uuid::Uuid,
    #[builder(setter(custom))]
    pub(super) address: Address,
    #[builder(setter(into))]
    pub(super) address_idx: u32,
    pub(super) account_id: AccountId,
    pub(super) wallet_id: WalletId,
    #[builder(setter(strip_option), default)]
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
        builder.db_uuid(uuid::Uuid::new_v4());
        builder
    }

    pub fn initial_events(self) -> EntityEvents<AddressEvent> {
        let mut events = EntityEvents::init([
            AddressEvent::Initialized {
                db_uuid: self.db_uuid,
                account_id: self.account_id,
                wallet_id: self.wallet_id,
                keychain_id: self.keychain_id,
                profile_id: self.profile_id,
                address: self.address,
                address_idx: self.address_idx,
                kind: self.kind,
            },
            AddressEvent::ExternalIdUpdated {
                external_id: self.external_id,
            },
        ]);
        if let Some(metadata) = self.metadata {
            events.push(AddressEvent::MetadataUpdated { metadata })
        }
        events
    }
}

impl NewAddressBuilder {
    pub fn address(&mut self, address: Address) -> &mut Self {
        if self.external_id.is_none() {
            self.external_id = Some(address.to_string());
        }
        self.address = Some(address);
        self
    }
}

impl TryFrom<EntityEvents<AddressEvent>> for WalletAddress {
    type Error = EntityError;

    fn try_from(events: EntityEvents<AddressEvent>) -> Result<Self, EntityError> {
        let mut builder = WalletAddressBuilder::default();
        for event in events.iter() {
            match event {
                AddressEvent::Initialized {
                    db_uuid,
                    account_id,
                    wallet_id,
                    address,
                    kind,
                    ..
                } => {
                    builder = builder
                        .db_uuid(*db_uuid)
                        .account_id(*account_id)
                        .address(address.clone())
                        .wallet_id(*wallet_id)
                        .kind(*kind);
                }
                AddressEvent::ExternalIdUpdated { external_id } => {
                    builder = builder.external_id(external_id.to_owned());
                }
                _ => {}
            }
        }
        builder.events(events).build()
    }
}
