use derive_builder::Builder;
use es_entity::*;
use serde::{Deserialize, Serialize};

use super::{error::XpubError, signer_config::*, signing_client::*, value::XPub as XPubValue};
use crate::primitives::*;

#[derive(EsEvent, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[es_event(id = "XpubId")]
pub enum XpubEvent {
    Initialized {
        db_uuid: XpubId,
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

#[derive(EsEntity, Builder)]
#[builder(pattern = "owned", build_fn(error = "EsEntityError"))]
pub struct Xpub {
    pub account_id: AccountId,
    pub name: String,
    pub value: XPubValue,
    pub original: String,
    #[builder(default)]
    pub(super) encrypted_signer_config: Option<(ConfigCyper, Nonce)>,
    pub(super) id: XpubId,
    pub(super) events: EntityEvents<XpubEvent>,
}

impl Xpub {
    pub fn id(&self) -> XPubId {
        self.value.id()
    }

    pub fn set_signer_config(
        &mut self,
        config: SignerConfig,
        secret: &EncryptionKey,
    ) -> Result<(), XpubError> {
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
pub struct NewXpub {
    pub(super) id: XpubId,
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) name: String,
    pub(super) original: String,
    pub(super) value: XPubValue,
    pub(super) fingerprint: XPubId,
}

impl NewXpub {
    pub fn builder() -> NewXpubBuilder {
        let mut builder = NewXpubBuilder::default();
        builder.id(XpubId::new());
        builder
    }

    pub fn id(&self) -> XPubId {
        self.value.id()
    }
}
impl IntoEvents<XpubEvent> for NewXpub {
    fn into_events(self) -> EntityEvents<XpubEvent> {
        let xpub = self.value.inner;
        let events = vec![
            XpubEvent::Initialized {
                db_uuid: self.id,
                account_id: self.account_id,
                fingerprint: xpub.fingerprint(),
                parent_fingerprint: xpub.parent_fingerprint,
                xpub,
                original: self.original,
                derivation_path: self.value.derivation,
            },
            XpubEvent::NameUpdated { name: self.name },
        ];
        EntityEvents::init(self.id, events)
    }
}

impl TryFromEvents<XpubEvent> for Xpub {
    fn try_from_events(events: EntityEvents<XpubEvent>) -> Result<Self, EsEntityError> {
        let mut builder = XpubBuilder::default();
        for event in events.iter_all() {
            match event {
                XpubEvent::Initialized {
                    db_uuid,
                    account_id,
                    xpub,
                    derivation_path,
                    original,
                    ..
                } => {
                    builder = builder
                        .id(*db_uuid)
                        .account_id(*account_id)
                        .value(XPubValue {
                            inner: *xpub,
                            derivation: derivation_path.as_ref().cloned(),
                        })
                        .original(original.clone());
                }
                XpubEvent::NameUpdated { name } => {
                    builder = builder.name(name.clone());
                }
            }
        }
        builder.events(events).build()
    }
}
