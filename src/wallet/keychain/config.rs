use bdk::{descriptor::DescriptorPublicKey, miniscript::ForEachKey};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};

use crate::{primitives::bitcoin::ExtendedDescriptor, xpub::*};

#[derive(Deserialize, Serialize, PartialEq, Eq, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)]
pub enum KeychainConfig {
    Wpkh {
        xpub: XPub,
    },
    Descriptors {
        internal: ExtendedDescriptor,
        external: ExtendedDescriptor,
    },
    SortedMultisig {
        xpub: Vec<XPub>,
        threshold: u32,
    },
}

impl KeychainConfig {
    pub fn wpkh(xpub: XPub) -> Self {
        Self::Wpkh { xpub }
    }

    pub fn sorted_multisig(xpub: Vec<XPub>, threshold: u32) -> Self {
        Self::SortedMultisig { xpub, threshold }
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
            Self::SortedMultisig { xpub, .. } => xpub.clone(),
        }
    }

    pub fn external_descriptor(&self) -> ExtendedDescriptor {
        match self {
            Self::Wpkh { xpub } => format!("wpkh({}/0/*)", xpub)
                .parse()
                .expect("Couldn't create internal wpkh descriptor"),
            Self::Descriptors { external, .. } => external.clone(),
            Self::SortedMultisig { xpub, threshold } => {
                let keys = xpub
                    .iter()
                    .map(|xpub| format!("{}/0/*", xpub))
                    .collect::<Vec<_>>();
                let keys = keys.join(",");
                println!("{}", keys);
                format!("wsh(sortedmulti({},{}))", threshold, keys)
                    .parse()
                    .expect("Couldn't create external sorted multisig descriptor")
            }
        }
    }

    pub fn internal_descriptor(&self) -> ExtendedDescriptor {
        match self {
            Self::Wpkh { xpub } => format!("wpkh({}/1/*)", xpub)
                .parse()
                .expect("Couldn't create internal wpkh descriptor"),
            Self::Descriptors { internal, .. } => internal.clone(),
            Self::SortedMultisig { xpub, threshold } => {
                let keys = xpub
                    .iter()
                    .map(|xpub| format!("{}/1/*", xpub))
                    .collect::<Vec<_>>();
                let keys = keys.join(",");
                format!("wsh(sortedmulti({},{}))", threshold, keys)
                    .parse()
                    .expect("Couldn't create internal sorted multisig descriptor")
            }
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
