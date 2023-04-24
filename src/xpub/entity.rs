use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use super::{signing_client::*, value::XPub as XPubValue};
use crate::{entity::*, primitives::*};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignerConfig {
    Lnd(LndSignerConfig),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum XPubEvent {
    // Spelling is Xpub for nicer serialization (not x_pub_initialized)
    XpubInitialized,
    SignerConfigUpdated { config: SignerConfig },
}

pub struct AccountXPub {
    pub account_id: AccountId,
    pub key_name: String,
    pub value: XPubValue,
    pub(super) db_uuid: uuid::Uuid,
    pub(super) events: EntityEvents<XPubEvent>,
}

impl AccountXPub {
    pub fn id(&self) -> XPubId {
        self.value.id()
    }

    pub fn set_signer_config(&mut self, config: SignerConfig) {
        self.events.push(XPubEvent::SignerConfigUpdated { config })
    }

    fn signing_cfg(&self) -> Option<&SignerConfig> {
        let mut ret = None;
        for event in self.events.iter() {
            if let XPubEvent::SignerConfigUpdated { config } = event {
                ret = Some(config)
            }
        }
        ret
    }

    pub async fn remote_signing_client(
        &self,
    ) -> Result<Option<Box<dyn RemoteSigningClient + 'static>>, SigningClientError> {
        let client = match self.signing_cfg() {
            Some(SignerConfig::Lnd(ref cfg)) => {
                let client = LndRemoteSigner::connect(cfg).await?;
                Some(Box::new(client) as Box<dyn RemoteSigningClient + 'static>)
            }
            _ => None,
        };
        Ok(client)
    }
}

#[derive(Builder, Clone, Debug)]
pub struct NewXPub {
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) key_name: String,
    pub(super) value: XPubValue,
}

impl NewXPub {
    pub fn builder() -> NewXPubBuilder {
        NewXPubBuilder::default()
    }

    pub fn id(&self) -> XPubId {
        self.value.id()
    }

    pub(super) fn initial_events() -> EntityEvents<XPubEvent> {
        EntityEvents::init([XPubEvent::XpubInitialized])
    }
}
