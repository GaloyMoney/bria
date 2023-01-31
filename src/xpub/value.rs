use bitcoin::util::bip32::{DerivationPath, ExtendedPubKey};
use serde::{Deserialize, Serialize};
use std::fmt;

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

impl<O: Into<String>, D: AsRef<str>> TryFrom<(O, Option<D>)> for XPub {
    type Error = BriaError;

    fn try_from((original, derivation): (O, Option<D>)) -> Result<Self, Self::Error> {
        let original = original.into();
        let derivation: Option<DerivationPath> = derivation.map(|d| d.as_ref().parse().unwrap());
        let inner: ExtendedPubKey = original.parse()?;
        if let Some(ref d) = derivation {
            if d.len() != inner.depth as usize {
                return Err(BriaError::XPubDepthMismatch(inner.depth, d.len()));
            }
        } else if inner.depth > 0 {
            return Err(BriaError::XPubDepthMismatch(inner.depth, 0));
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
}
