use derive_builder::Builder;
use es_entity::*;
use serde::{Deserialize, Serialize};

use crate::primitives::{bitcoin::*, *};

#[derive(EsEvent, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[es_event(id = "uuid::Uuid")]
pub enum WalletAddressEvent {
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

#[derive(EsEntity, Builder)]
#[builder(pattern = "owned", build_fn(error = "EsEntityError"))]
pub struct WalletAddress {
    pub account_id: AccountId,
    pub address: Address,
    pub wallet_id: WalletId,
    pub external_id: String,
    kind: KeychainKind,
    pub(super) id: uuid::Uuid,
    pub(super) events: EntityEvents<WalletAddressEvent>,
}

impl std::fmt::Debug for WalletAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("WalletAddress")
            .field("account_id", &self.account_id)
            .field("address", &self.address)
            .field("wallet_id", &self.wallet_id)
            .field("external_id", &self.external_id)
            .field("kind", &self.kind)
            .field("id", &self.id)
            .finish()
    }
}

impl WalletAddress {
    pub fn metadata(&self) -> Option<&serde_json::Value> {
        let mut ret = None;
        for event in self.events.iter_all() {
            if let WalletAddressEvent::MetadataUpdated { metadata } = event {
                ret = Some(metadata)
            }
        }
        ret
    }

    pub fn update_external_id(&mut self, external_id: String) {
        if self.external_id != external_id {
            self.external_id.clone_from(&external_id);
            self.events
                .push(WalletAddressEvent::ExternalIdUpdated { external_id });
        }
    }

    pub fn update_metadata(&mut self, metadata: serde_json::Value) {
        if self.metadata() != Some(&metadata) {
            self.events
                .push(WalletAddressEvent::MetadataUpdated { metadata });
        }
    }

    pub fn is_external(&self) -> bool {
        self.kind == KeychainKind::External
    }
}

#[derive(Builder, Clone, Debug)]
pub struct NewWalletAddress {
    pub(super) id: uuid::Uuid,
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

impl NewWalletAddress {
    pub fn builder() -> NewWalletAddressBuilder {
        let mut builder = NewWalletAddressBuilder::default();
        builder.id(uuid::Uuid::new_v4());
        builder
    }
}

impl IntoEvents<WalletAddressEvent> for NewWalletAddress {
    fn into_events(self) -> EntityEvents<WalletAddressEvent> {
        let mut events = vec![
            WalletAddressEvent::Initialized {
                db_uuid: self.id,
                account_id: self.account_id,
                wallet_id: self.wallet_id,
                keychain_id: self.keychain_id,
                profile_id: self.profile_id,
                address: self.address,
                address_idx: self.address_idx,
                kind: self.kind,
            },
            WalletAddressEvent::ExternalIdUpdated {
                external_id: self.external_id,
            },
        ];
        if let Some(metadata) = self.metadata {
            events.push(WalletAddressEvent::MetadataUpdated { metadata })
        }
        EntityEvents::init(self.id, events)
    }
}

impl NewWalletAddressBuilder {
    pub fn address(&mut self, address: Address) -> &mut Self {
        if self.external_id.is_none() {
            self.external_id = Some(address.to_string());
        }
        self.address = Some(address);
        self
    }
}

impl TryFromEvents<WalletAddressEvent> for WalletAddress {
    fn try_from_events(events: EntityEvents<WalletAddressEvent>) -> Result<Self, EsEntityError> {
        let mut builder = WalletAddressBuilder::default();
        for event in events.iter_all() {
            match event {
                WalletAddressEvent::Initialized {
                    db_uuid,
                    account_id,
                    wallet_id,
                    address,
                    kind,
                    ..
                } => {
                    builder = builder
                        .id(*db_uuid)
                        .account_id(*account_id)
                        .address(address.clone())
                        .wallet_id(*wallet_id)
                        .kind(*kind);
                }
                WalletAddressEvent::ExternalIdUpdated { external_id } => {
                    builder = builder.external_id(external_id.to_owned());
                }
                _ => {}
            }
        }
        builder.events(events).build()
    }
}
