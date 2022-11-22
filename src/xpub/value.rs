use bitcoin::util::bip32::ExtendedPubKey;
use serde::{Deserialize, Serialize};

use crate::{error::*, primitives::XPubId};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct XPub {
    pub(super) original: String,
    pub(super) inner: ExtendedPubKey,
}

impl XPub {
    pub fn id(&self) -> XPubId {
        XPubId::from(self.inner.fingerprint())
    }
}

impl std::ops::Deref for XPub {
    type Target = ExtendedPubKey;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::str::FromStr for XPub {
    type Err = BriaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            original: s.to_string(),
            inner: s.parse()?,
        })
    }
}
