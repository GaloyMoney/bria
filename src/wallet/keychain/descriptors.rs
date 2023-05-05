use bdk::{
    descriptor::{DescriptorPublicKey, ExtendedDescriptor},
    miniscript::ForEachKey,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};

use crate::xpub::*;

#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub struct WalletKeychainDescriptors {
    pub internal: ExtendedDescriptor,
    pub external: ExtendedDescriptor,
}

impl WalletKeychainDescriptors {
    pub fn wpkh(xpub: XPub) -> Self {
        let external = format!("wpkh({}/0/*)", xpub)
            .parse()
            .expect("Couldn't create external wpkh descriptor");
        let internal = format!("wpkh({}/1/*)", xpub)
            .parse()
            .expect("Couldn't create internal wpkh descriptor");
        Self { internal, external }
    }

    pub fn xpubs(&self) -> Vec<XPub> {
        let mut ret = HashMap::new();
        self.internal.for_each_key(|key| {
            let xpub = XPub::try_from(key).expect("Couldn't convert xpub");
            ret.insert(xpub.id(), xpub);
            true
        });
        self.external.for_each_key(|key| {
            let xpub = XPub::try_from(key).expect("Couldn't convert xpub");
            ret.insert(xpub.id(), xpub);
            true
        });
        ret.into_values().collect()
    }
}

impl TryFrom<(&str, &str)> for WalletKeychainDescriptors {
    type Error = crate::error::BriaError;

    fn try_from((external, internal): (&str, &str)) -> Result<Self, Self::Error> {
        let external = ExtendedDescriptor::from_str(external)?;
        let internal = ExtendedDescriptor::from_str(internal)?;
        external.sanity_check()?;
        internal.sanity_check()?;
        if external.for_any_key(|key| matches!(key, DescriptorPublicKey::Single(_)))
            || internal.for_any_key(|key| matches!(key, DescriptorPublicKey::Single(_)))
        {
            return Err(crate::error::BriaError::UnsupportedPubKeyType);
        }
        Ok(Self { internal, external })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keychain_descriptors() {
        let external = "wpkh([1ff51810/84'/0'/0']tpubDDdzmt7vndmNywiVAeBPuhYLTFa7hmtfaqUxxTv5iLy7bxU93B62M9WKFSmn1BEN2vte8GDD3SUNKbupRajFW4RK8hd3i6W15pvTRQfo1fK/0/*)#q8r69l4d";
        let internal = "wpkh([1ff51810/84'/0'/0']tpubDDdzmt7vndmNywiVAeBPuhYLTFa7hmtfaqUxxTv5iLy7bxU93B62M9WKFSmn1BEN2vte8GDD3SUNKbupRajFW4RK8hd3i6W15pvTRQfo1fK/1/*)#3nxmc294";
        let descriptor = WalletKeychainDescriptors::try_from((external, internal))
            .expect("Couldn't parse descriptor");
        let xpubs = descriptor.xpubs();
        assert!(xpubs.len() == 1);

        let other = "wpkh([37230651/84'/0'/0']tpubDDt6ytR6KpwWLoLKT41nHzuoK9GWNtK3Hxhsgobwqj7H9ENyTxbT2he9q43mUm17LTXz6KqQkPHzvRmd1XBX1k1naVJnAvKVs8tayskct6f/1/*)#gj3xrn0h";
        let descriptor = WalletKeychainDescriptors::try_from((external, other))
            .expect("Couldn't parse descriptor");
        let xpubs = descriptor.xpubs();
        assert!(xpubs.len() == 2);
    }
}
