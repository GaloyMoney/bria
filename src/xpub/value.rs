use bitcoin::util::bip32::{DerivationPath, ExtendedPubKey};
use serde::{Deserialize, Serialize};

use crate::{error::*, primitives::XPubId};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct XPub {
    pub(super) derivation: Option<DerivationPath>,
    pub(super) original: String,
    pub(super) inner: ExtendedPubKey,
}

impl XPub {
    pub fn id(&self) -> XPubId {
        XPubId::from(self.inner.fingerprint())
    }
}

impl<O: Into<String>, D: AsRef<str>> TryFrom<(O, Option<D>)> for XPub {
    type Error = BriaError;

    fn try_from((original, derivation): (O, Option<D>)) -> Result<Self, Self::Error> {
        let original = original.into();
        let derivation: Option<DerivationPath> = derivation.map(|d| d.as_ref().parse().unwrap());
        let inner: ExtendedPubKey = original.parse()?;
        if let Some(ref d) = derivation {
            if d.len() != inner.depth as usize {
                return Err(BriaError::XPubDepthMissmatch(inner.depth, d.len()));
            }
        } else {
            if inner.depth > 0 {
                return Err(BriaError::XPubDepthMissmatch(inner.depth, 0));
            }
        }

        Ok(Self {
            derivation,
            original,
            inner,
        })
    }
}

impl std::ops::Deref for XPub {
    type Target = ExtendedPubKey;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
