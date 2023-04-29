use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

pub use sqlx_ledger::{
    event::SqlxLedgerEventId, AccountId as LedgerAccountId, JournalId as LedgerJournalId,
    TransactionId as LedgerTransactionId,
};
crate::entity_id! { AdminApiKeyId }
crate::entity_id! { AccountId }
impl From<LedgerJournalId> for AccountId {
    fn from(id: LedgerJournalId) -> Self {
        Self::from(uuid::Uuid::from(id))
    }
}
impl From<AccountId> for LedgerJournalId {
    fn from(id: AccountId) -> Self {
        Self::from(uuid::Uuid::from(id))
    }
}
crate::entity_id! { ProfileId }
crate::entity_id! { ProfileApiKeyId }
crate::entity_id! { SigningSessionId }
crate::entity_id! { KeychainId }
crate::entity_id! { SignerId }
crate::entity_id! { WalletId }
crate::entity_id! { AddressId }
crate::entity_id! { BatchGroupId }
crate::entity_id! { PayoutId }
crate::entity_id! { BatchId }
crate::entity_id! { OutboxEventId }

#[derive(Debug, Clone, Hash, PartialEq, Eq, Copy, Serialize, Deserialize)]
#[serde(transparent)]
pub struct XPubId(bitcoin::Fingerprint);

impl From<bitcoin::Fingerprint> for XPubId {
    fn from(fp: bitcoin::Fingerprint) -> Self {
        Self(fp)
    }
}

impl std::str::FromStr for XPubId {
    type Err = <bitcoin::Fingerprint as std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fingerprint = bitcoin::Fingerprint::from_str(s)?;
        Ok(Self(fingerprint))
    }
}

impl std::ops::Deref for XPubId {
    type Target = bitcoin::Fingerprint;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub mod bitcoin {
    pub use bdk::{
        bitcoin::{
            blockdata::{
                script::Script,
                transaction::{OutPoint, Transaction, TxOut},
            },
            consensus,
            hash_types::Txid,
            util::{
                address::Error as AddressError,
                bip32::{self, DerivationPath, ExtendedPubKey, Fingerprint},
                psbt,
            },
            Address, Network,
        },
        BlockTime, KeychainKind,
    };
    pub mod pg {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
        #[sqlx(type_name = "KeychainKind", rename_all = "snake_case")]
        pub enum PgKeychainKind {
            External,
            Internal,
        }
        impl From<super::KeychainKind> for PgKeychainKind {
            fn from(kind: super::KeychainKind) -> Self {
                match kind {
                    super::KeychainKind::External => Self::External,
                    super::KeychainKind::Internal => Self::Internal,
                }
            }
        }
        impl From<PgKeychainKind> for super::KeychainKind {
            fn from(kind: PgKeychainKind) -> Self {
                match kind {
                    PgKeychainKind::External => Self::External,
                    PgKeychainKind::Internal => Self::Internal,
                }
            }
        }
    }
}

pub const SATS_PER_BTC: Decimal = dec!(100_000_000);

#[derive(Debug, Clone, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum TxPriority {
    NextBlock,
    OneHour,
    Economy,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct Satoshis(Decimal);

impl Satoshis {
    pub const ZERO: Self = Self(Decimal::ZERO);
    pub const ONE: Self = Self(Decimal::ONE);

    pub fn to_btc(self) -> Decimal {
        self.0 / SATS_PER_BTC
    }

    pub fn from_btc(btc: Decimal) -> Self {
        Self(btc * SATS_PER_BTC)
    }

    pub fn into_inner(self) -> Decimal {
        self.0
    }

    pub fn flip_sign(self) -> Self {
        Self(self.0 * Decimal::NEGATIVE_ONE)
    }

    pub fn abs(&self) -> Self {
        Self(self.0.abs())
    }
}

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

impl From<Satoshis> for u64 {
    fn from(sats: Satoshis) -> u64 {
        sats.0.to_u64().expect("Couldn't convert Satoshis")
    }
}

impl From<i32> for Satoshis {
    fn from(sats: i32) -> Self {
        Self(Decimal::from(sats))
    }
}

impl From<u32> for Satoshis {
    fn from(sats: u32) -> Self {
        Self(Decimal::from(sats))
    }
}

impl From<i64> for Satoshis {
    fn from(sats: i64) -> Self {
        Self(Decimal::from(sats as u64))
    }
}

impl From<Satoshis> for i64 {
    fn from(sats: Satoshis) -> i64 {
        sats.0.to_i64().expect("Couldn't convert Satoshis")
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

impl std::ops::Mul<i32> for Satoshis {
    type Output = Satoshis;
    fn mul(self, rhs: i32) -> Self {
        self * Satoshis::from(rhs)
    }
}

impl std::ops::Div<Satoshis> for Satoshis {
    type Output = Satoshis;
    fn div(self, rhs: Satoshis) -> Self {
        Self(self.0 / rhs.0)
    }
}

impl std::ops::AddAssign<Satoshis> for Satoshis {
    fn add_assign(&mut self, rhs: Satoshis) {
        *self = Self(self.0 + rhs.0)
    }
}
