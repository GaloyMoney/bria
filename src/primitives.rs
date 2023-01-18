use bitcoin::util::bip32::Fingerprint;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

crate::entity_id! { AdminApiKeyId }
crate::entity_id! { AccountId }
crate::entity_id! { AccountApiKeyId }
crate::entity_id! { KeychainId }
crate::entity_id! { SignerId }
crate::entity_id! { WalletId }
crate::entity_id! { BatchGroupId }
crate::entity_id! { PayoutId }
crate::entity_id! { BatchId }

pub struct XPubId(Fingerprint);

impl From<Fingerprint> for XPubId {
    fn from(fp: Fingerprint) -> Self {
        Self(fp)
    }
}

impl std::ops::Deref for XPubId {
    type Target = Fingerprint;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub const SATS_PER_BTC: Decimal = dec!(100_000_000);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxPriority {
    NextBlock,
    OneHour,
    Economy,
}

#[derive(Debug, Clone, Copy)]
pub struct Satoshis(Decimal);

impl From<Decimal> for Satoshis {
    fn from(sats: Decimal) -> Self {
        Self(sats)
    }
}

impl From<u64> for Satoshis {
    fn from(sats: u64) -> Self {
        Self(Decimal::from(sats))
    }
}

impl std::ops::Add<Satoshis> for Satoshis {
    type Output = Satoshis;
    fn add(self, rhs: Satoshis) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub<Satoshis> for Satoshis {
    type Output = Satoshis;
    fn sub(self, rhs: Satoshis) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul<Satoshis> for Satoshis {
    type Output = Satoshis;
    fn mul(self, rhs: Satoshis) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl std::ops::Div<Satoshis> for Satoshis {
    type Output = Satoshis;
    fn div(self, rhs: Satoshis) -> Self {
        Self(self.0 / rhs.0)
    }
}
