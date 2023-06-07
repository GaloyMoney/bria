use bdk::{descriptor::DescriptorPublicKey, miniscript::ForEachKey};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};

use crate::{primitives::bitcoin::ExtendedDescriptor, xpub::*};

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeychainConfig {
    Wpkh {
        xpub: XPub,
    },
    Descriptors {
        internal: ExtendedDescriptor,
        external: ExtendedDescriptor,
    },
}

impl KeychainConfig {
    pub fn wpkh(xpub: XPub) -> Self {
        Self::Wpkh { xpub }
    }

    pub fn xpubs(&self) -> Vec<XPub> {
        match self {
            Self::Wpkh { xpub } => vec![xpub.clone()],
            Self::Descriptors { internal, external } => {
                let mut ret = HashMap::new();
                internal.for_each_key(|key| {
                    let xpub = XPub::try_from(key).expect("Couldn't convert xpub");
                    ret.insert(xpub.id(), xpub);
                    true
                });
                external.for_each_key(|key| {
                    let xpub = XPub::try_from(key).expect("Couldn't convert xpub");
                    ret.insert(xpub.id(), xpub);
                    true
                });
                ret.into_values().collect()
            }
        }
    }

    pub fn external_descriptor(&self) -> ExtendedDescriptor {
        match self {
            Self::Wpkh { xpub } => format!("wpkh({}/0/*)", xpub)
                .parse()
                .expect("Couldn't create internal wpkh descriptor"),
            Self::Descriptors { external, .. } => external.clone(),
        }
    }

    pub fn internal_descriptor(&self) -> ExtendedDescriptor {
        match self {
            Self::Wpkh { xpub } => format!("wpkh({}/1/*)", xpub)
                .parse()
                .expect("Couldn't create internal wpkh descriptor"),
            Self::Descriptors { internal, .. } => internal.clone(),
        }
    }
}

impl TryFrom<(&str, &str)> for KeychainConfig {
    type Error = crate::wallet::error::WalletError;

    fn try_from((external, internal): (&str, &str)) -> Result<Self, Self::Error> {
        let external = ExtendedDescriptor::from_str(external)?;
        let internal = ExtendedDescriptor::from_str(internal)?;
        external.sanity_check()?;
        internal.sanity_check()?;
        if external.for_any_key(|key| matches!(key, DescriptorPublicKey::Single(_)))
            || internal.for_any_key(|key| matches!(key, DescriptorPublicKey::Single(_)))
        {
            return Err(crate::wallet::error::WalletError::UnsupportedPubKeyType);
        }
        Ok(Self::Descriptors { internal, external })
    }
}
