use bitcoin::util::bip32::ExtendedPubKey;
use serde::{Deserialize, Serialize};

use crate::{error::*, primitives::XPubId};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct XPub(pub(super) ExtendedPubKey);

impl XPub {
    pub fn id(&self) -> XPubId {
        XPubId::from(self.0.fingerprint())
    }
}

impl std::ops::Deref for XPub {
    type Target = ExtendedPubKey;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for XPub {
    type Err = BriaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}
