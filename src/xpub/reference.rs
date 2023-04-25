use crate::primitives::{bitcoin, *};

pub enum XPubRef {
    Id(XPubId),
    Name(String),
    Key(bitcoin::ExtendedPubKey),
}

impl std::str::FromStr for XPubRef {
    type Err = crate::error::BriaError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(id) = XPubId::from_str(s) {
            Ok(XPubRef::Id(id))
        } else if let Ok(value) = bitcoin::ExtendedPubKey::from_str(s) {
            Ok(XPubRef::Key(value))
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
