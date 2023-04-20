use derive_builder::Builder;

use super::{signer::SignerConfig, signing_client::*, value::XPub as XPubValue};
use crate::primitives::*;

pub struct AccountXPub {
    pub account_id: AccountId,
    pub key_name: String,
    pub value: XPubValue,
    pub(super) signing_cfg: Option<SignerConfig>,
}

impl AccountXPub {
    pub fn id(&self) -> XPubId {
        self.value.id()
    }

    pub async fn remote_signing_client(
        &self,
    ) -> Result<Option<Box<dyn RemoteSigningClient + 'static>>, SigningClientError> {
        let client = match self.signing_cfg {
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
}
