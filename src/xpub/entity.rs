use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use super::{signing_client::*, value::XPub as XPubValue};
use crate::{entity::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignerConfig {
    Lnd(LndSignerConfig),
    Bitcoind(BitcoindSignerConfig),
}

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
    SignerConfigUpdated {
        encrypted_config: Vec<u8>,
    },
}

#[derive(Builder)]
#[builder(pattern = "owned", build_fn(error = "EntityError"))]
pub struct AccountXPub {
    pub account_id: AccountId,
    pub key_name: String,
    pub value: XPubValue,
    pub original: String,
    pub(super) db_uuid: uuid::Uuid,
    pub(super) events: EntityEvents<XPubEvent>,
}

impl AccountXPub {
    pub fn id(&self) -> XPubId {
        self.value.id()
    }

    fn encrypt_config(&mut self, config: SignerConfig, key: &[u8], nonce: &[u8]) -> Vec<u8> {
        // serialize the config
        let config_vec = serde_json::to_vec(&config).unwrap();
        let config_bytes = config_vec.as_slice();
        let key_bytes = Key::from_slice(key);
        let cipher = ChaCha20Poly1305::new(key_bytes);
        let nonce_bytes = Nonce::from_slice(nonce);
        let ciphertext: Vec<u8> = cipher.encrypt(&nonce_bytes, config_bytes).unwrap();
        return ciphertext;
    }

    pub fn set_signer_config(&mut self, config: SignerConfig, key: Vec<u8>, nonce: Vec<u8>) {
        let key_bytes = key.as_slice();
        let nonce_bytes = nonce.as_slice();
        let encrypted_config = self.encrypt_config(config, key_bytes, nonce_bytes);
        self.events
            .push(XPubEvent::SignerConfigUpdated { encrypted_config });
    }

    pub fn signing_cfg(&self) -> Option<SignerConfig> {
        let mut ret = None;
        for event in self.events.iter() {
            if let XPubEvent::SignerConfigUpdated { encrypted_config } = event {
                let config = self.decrypt_config(encrypted_config.clone());
                ret = Some(config);
            }
        }
        ret
    }

    fn decrypt_config(&self, encrypted_config: Vec<u8>) -> SignerConfig {
        // let key: [u8; 32] = [
        //     0x45, 0x54, 0x82, 0x41, 0x08, 0x1a, 0xa3, 0x91, 0x56, 0xa2, 0xd2, 0x14, 0x35, 0x0a,
        //     0x0f, 0x50, 0xc9, 0x18, 0x2e, 0x0e, 0x50, 0x3c, 0x4e, 0xd6, 0x8d, 0x6a, 0xb5, 0xe4,
        //     0x2f, 0x0a, 0x08, 0x77,
        // ];

        // let config: SignerConfig = serde_json::from_slice(&decrypted_config).unwrap();
        // return config;
        unimplemented!()
    }

    pub fn has_signer_config(&self) -> bool {
        self.signing_cfg().is_some()
    }

    pub fn derivation_path(&self) -> Option<bitcoin::DerivationPath> {
        self.value.derivation.clone()
    }

    pub async fn remote_signing_client(
        &self,
    ) -> Result<Option<Box<dyn RemoteSigningClient + 'static>>, SigningClientError> {
        let client = match self.signing_cfg() {
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

impl TryFrom<EntityEvents<XPubEvent>> for AccountXPub {
    type Error = EntityError;
    fn try_from(events: EntityEvents<XPubEvent>) -> Result<Self, Self::Error> {
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
                _ => (),
            }
        }
        builder.events(events).build()
    }
}
