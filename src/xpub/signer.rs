use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use super::{signing_client::LndSignerConfig, value::XPub as XPubValue};
use crate::primitives::*;

#[derive(Builder, Debug, Clone)]
pub struct NewSigner {
    pub(super) id: SignerId,
    pub(super) xpub: XPubValue,
    pub(super) config: SignerConfig,
}

impl NewSigner {
    pub fn builder() -> NewSignerBuilder {
        let mut builder = NewSignerBuilder::default();
        builder.id(SignerId::new());
        builder
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignerConfig {
    Lnd(LndSignerConfig),
}
