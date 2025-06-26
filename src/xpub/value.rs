use bdk::descriptor::DescriptorPublicKey;
use serde::{Deserialize, Serialize};
use std::fmt;

use super::error::XPubError;
use crate::primitives::{
    bitcoin::{DerivationPath, ExtendedPubKey},
    XPubFingerprint,
};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
pub struct XPub {
    pub(super) derivation: Option<DerivationPath>,
    pub(super) inner: ExtendedPubKey,
}

impl XPub {
    pub fn fingerprint(&self) -> XPubFingerprint {
        XPubFingerprint::from(self.inner.fingerprint())
    }

    pub fn inner(&self) -> &ExtendedPubKey {
        &self.inner
    }
}

impl fmt::Display for XPub {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref d) = self.derivation {
            write!(f, "[{}", self.parent_fingerprint)?;
            for child in d {
                write!(f, "/{child}")?;
            }
            f.write_str("]")?;
        }
        write!(f, "{}", self.inner)?;
        Ok(())
    }
}

impl TryFrom<&DescriptorPublicKey> for XPub {
    type Error = XPubError;

    fn try_from(pk: &DescriptorPublicKey) -> Result<Self, Self::Error> {
        let derivation_path = pk.full_derivation_path();
        match pk {
            DescriptorPublicKey::Single(_) => Err(XPubError::UnsupportedPubKeyType),
            DescriptorPublicKey::XPub(inner) => Ok(Self {
                derivation: derivation_path,
                inner: inner.xkey,
            }),
            DescriptorPublicKey::MultiXPub(_) => Err(XPubError::UnsupportedPubKeyType),
        }
    }
}

impl<O: AsRef<str>, D: AsRef<str>> TryFrom<(O, Option<D>)> for XPub {
    type Error = XPubError;

    fn try_from((original, derivation): (O, Option<D>)) -> Result<Self, Self::Error> {
        let derivation: Option<DerivationPath> =
            derivation.map(|d| d.as_ref().parse()).transpose()?;
        use bdk::bitcoin::base58;
        let mut xpub_data =
            base58::decode_check(original.as_ref()).map_err(XPubError::XPubParseError)?;
        fix_version_bits_for_rust_bitcoin(&mut xpub_data);
        let inner = ExtendedPubKey::decode(&xpub_data)?;
        if let Some(ref d) = derivation {
            if d.len() != inner.depth as usize {
                return Err(XPubError::XPubDepthMismatch(inner.depth, d.len()));
            }
        } else if inner.depth > 0 {
            return Err(XPubError::XPubDepthMismatch(inner.depth, 0));
        }

        Ok(Self { derivation, inner })
    }
}

impl std::ops::Deref for XPub {
    type Target = ExtendedPubKey;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

fn fix_version_bits_for_rust_bitcoin(data: &mut [u8]) {
    match data {
        // zpub => xpub
        [0x04u8, 0xB2, 0x47, 0x46, ..] => {
            data[0] = 0x04u8;
            data[1] = 0x88;
            data[2] = 0xB2;
            data[3] = 0x1E;
        }
        // Transfer vpub => tpub
        [0x04u8, 0x5F, 0x1C, 0xF6, ..] => {
            data[0] = 0x04u8;
            data[1] = 0x35;
            data[2] = 0x87;
            data[3] = 0xCF;
        }
        _ => (),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_string() {
        let xpub = XPub::try_from(
            ("tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4",
             Some("m/84'/0'/0'"))).unwrap();
        assert_eq!(xpub.to_string(),
        "[8df69d29/84'/0'/0']tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4");
    }

    #[test]
    fn test_import_vpub() {
        let original = "vpub5YdbDxAzXv4io9b5t4kRRFwLfhjFiFJAcUnDMbYGRLDHr5AzxFYBqa19AkkFfasDn9qXUuHBcw5JQWmE23GXahvuWixoLxsNe4Du85UGsp7";
        assert!(XPub::try_from((original, Some("m/84'/0'/0'"))).is_ok());
    }
}
