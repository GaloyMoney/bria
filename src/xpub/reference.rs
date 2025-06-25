use crate::primitives::*;

pub enum XPubRef {
    Id(XPubFingerprint),
    Name(String),
}

impl std::str::FromStr for XPubRef {
    type Err = super::error::XPubError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = XPubFingerprint::from_str(s) {
            Ok(XPubRef::Id(id))
        } else {
            Ok(XPubRef::Name(s.to_string()))
        }
    }
}

impl From<XPubFingerprint> for XPubRef {
    fn from(id: XPubFingerprint) -> Self {
        Self::Id(id)
    }
}

impl From<&XPubFingerprint> for XPubRef {
    fn from(id: &XPubFingerprint) -> Self {
        Self::Id(*id)
    }
}
