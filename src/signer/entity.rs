use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::primitives::*;

#[derive(Builder, Debug, Clone)]
pub struct NewSigner {
    pub(super) id: SignerId,
    pub(super) xpub_name: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndSignerConfig {
    endpoint: String,
    cert: String,
    macaroon: String,
}
