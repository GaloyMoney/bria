use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use super::{error::XPubError, signer_config::*, signing_client::*, value::XPub as XPubValue};
use crate::{entity::*, primitives::*};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum XPubEvent {
    Initialized {
        db_uuid: uuid::Uuid,
        account_id: AccountId,
        fingerprint: bitcoin::Fingerprint,
        parent_fingerprint: bitcoin::Fingerprint,
        original: String,
        xpub: bitcoin::ExtendedPubKey,
        derivation_path: Option<bitcoin::DerivationPath>,
    },
    NameUpdated {
        name: String,
    },
}

#[derive(Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct AccountXPub {
    pub account_id: AccountId,
    pub key_name: String,
    pub value: XPubValue,
    pub original: String,
    pub(super) encrypted_signer_config: Option<(ConfigCyper, Nonce)>,
    pub(super) db_uuid: uuid::Uuid,
    pub(super) events: EntityEvents<XPubEvent>,
}

impl AccountXPub {
    pub fn id(&self) -> XPubId {
        self.value.id()
    }

    pub fn set_signer_config(
        &mut self,
        config: SignerConfig,
        secret: &EncryptionKey,
    ) -> Result<(), XPubError> {
        self.encrypted_signer_config = Some(config.encrypt(secret)?);
        Ok(())
    }

    pub fn signing_cfg(&self, key: EncryptionKey) -> Option<SignerConfig> {
        self.encrypted_signer_config
            .as_ref()
            .and_then(|(cfg, nonce)| SignerConfig::decrypt(&key, cfg, nonce).ok())
    }

    pub fn has_signer_config(&self) -> bool {
        self.encrypted_signer_config.is_some()
    }

    pub fn derivation_path(&self) -> Option<bitcoin::DerivationPath> {
        self.value.derivation.clone()
    }

    pub async fn remote_signing_client(
        &self,
        key: EncryptionKey,
    ) -> Result<Option<Box<dyn RemoteSigningClient + 'static>>, SigningClientError> {
        let client = match self.signing_cfg(key) {
            Some(SignerConfig::Lnd(ref cfg)) => {
                let client = LndRemoteSigner::connect(cfg).await?;
                Some(Box::new(client) as Box<dyn RemoteSigningClient + 'static>)
            }
            Some(SignerConfig::Bitcoind(ref cfg)) => {
                let client = BitcoindRemoteSigner::connect(cfg).await?;
                Some(Box::new(client) as Box<dyn RemoteSigningClient + 'static>)
            }
            None => None,
        };
        Ok(client)
    }
}

#[derive(Builder, Clone, Debug)]
pub struct NewAccountXPub {
    pub(super) db_uuid: uuid::Uuid,
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) key_name: String,
    pub(super) original: String,
    pub(super) value: XPubValue,
}

impl NewAccountXPub {
    pub fn builder() -> NewAccountXPubBuilder {
        let mut builder = NewAccountXPubBuilder::default();
        builder.db_uuid(uuid::Uuid::new_v4());
        builder
    }

    pub fn id(&self) -> XPubId {
        self.value.id()
    }

    pub(super) fn initial_events(self) -> EntityEvents<XPubEvent> {
        let xpub = self.value.inner;
        EntityEvents::init([
            XPubEvent::Initialized {
                db_uuid: self.db_uuid,
                account_id: self.account_id,
                fingerprint: xpub.fingerprint(),
                parent_fingerprint: xpub.parent_fingerprint,
                xpub,
                original: self.original,
                derivation_path: self.value.derivation,
            },
            XPubEvent::NameUpdated {
                name: self.key_name,
            },
        ])
    }
}

impl TryFrom<(EntityEvents<XPubEvent>, Option<(ConfigCyper, Nonce)>)> for AccountXPub {
    type Error = EntityError;

    fn try_from(
        (events, config): (EntityEvents<XPubEvent>, Option<(ConfigCyper, Nonce)>),
    ) -> Result<Self, Self::Error> {
        let mut builder = AccountXPubBuilder::default();
        for event in events.iter() {
            match event {
                XPubEvent::Initialized {
                    db_uuid,
                    account_id,
                    xpub,
                    derivation_path,
                    original,
                    ..
                } => {
                    builder = builder
                        .db_uuid(*db_uuid)
                        .account_id(*account_id)
                        .value(XPubValue {
                            inner: *xpub,
                            derivation: derivation_path.as_ref().cloned(),
                        })
                        .original(original.clone());
                }
                XPubEvent::NameUpdated { name } => {
                    builder = builder.key_name(name.clone());
                }
            }
        }
        if let Some((encrypted_config, nonce)) = config {
            builder = builder.encrypted_signer_config(Some((encrypted_config, nonce)));
        } else {
            builder = builder.encrypted_signer_config(None);
        }
        builder.events(events).build()
    }
}
