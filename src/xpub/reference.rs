use crate::primitives::*;

pub enum XPubRef {
    Id(XPubId),
    Name(String),
}

impl std::str::FromStr for XPubRef {
    type Err = super::error::XPubError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = XPubId::from_str(s) {
            Ok(XPubRef::Id(id))
        } else {
            Ok(XPubRef::Name(s.to_string()))
        }
    }
}

impl From<XPubId> for XPubRef {
    fn from(id: XPubId) -> Self {
        Self::Id(id)
    }
}

impl From<&XPubId> for XPubRef {
    fn from(id: &XPubId) -> Self {
        Self::Id(*id)
    }
}
