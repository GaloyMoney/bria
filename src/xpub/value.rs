use bitcoin::util::bip32::ExtendedPubKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{error::*, primitives::XPubId};

lazy_static::lazy_static! {
    static ref PREFIXES: HashMap<&'static str, [u8; 4]> = {
        let mut m = HashMap::new();
        m.insert("upub", [0x04, 0x4A, 0x52, 0x62]);
        m.insert("tpub", [0x04, 0x4A, 0x52, 0x62]);
        m.insert("xpub", [0x04, 0x88, 0xB2, 0x1E]);
        m
    };
}

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
