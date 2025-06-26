use crate::primitives::*;

pub enum XPubRef {
    Fingerprint(XPubFingerprint),
    Name(String),
}

impl std::str::FromStr for XPubRef {
    type Err = super::error::XPubError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(fingerprint) = XPubFingerprint::from_str(s) {
            Ok(XPubRef::Fingerprint(fingerprint))
        } else {
            Ok(XPubRef::Name(s.to_string()))
        }
    }
}

impl From<XPubFingerprint> for XPubRef {
    fn from(fingerprint: XPubFingerprint) -> Self {
        Self::Fingerprint(fingerprint)
    }
}

impl From<&XPubFingerprint> for XPubRef {
    fn from(fingerprint: &XPubFingerprint) -> Self {
        Self::Fingerprint(*fingerprint)
    }
}
