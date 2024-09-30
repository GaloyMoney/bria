use rust_decimal::{prelude::ToPrimitive, Decimal};
use rust_decimal_macros::dec;
use serde::{Deserialize, Deserializer, Serialize};
pub use sqlx_ledger::{
    event::SqlxLedgerEventId, AccountId as LedgerAccountId, JournalId as LedgerJournalId,
    TransactionId as LedgerTransactionId,
};

use std::fmt;

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
crate::entity_id! { PayoutQueueId }
crate::entity_id! { PayoutId }

impl From<PayoutId> for LedgerTransactionId {
    fn from(id: PayoutId) -> Self {
        Self::from(uuid::Uuid::from(id))
    }
}

impl From<LedgerTransactionId> for PayoutId {
    fn from(id: LedgerTransactionId) -> Self {
        Self::from(uuid::Uuid::from(id))
    }
}
crate::entity_id! { BatchId }
crate::entity_id! { OutboxEventId }
crate::entity_id! { PayjoinProposalId }

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

impl fmt::Display for XPubId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
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
            address::{Error as AddressError, NetworkChecked, NetworkUnchecked},
            bip32::{self, DerivationPath, ExtendedPubKey, Fingerprint},
            blockdata::{
                script::{Script, ScriptBuf},
                transaction::{OutPoint, Transaction, TxOut},
            },
            consensus,
            hash_types::Txid,
            psbt, Address as BdkAddress, Amount, Network,
        },
        descriptor::ExtendedDescriptor,
        BlockTime, FeeRate, KeychainKind,
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

#[derive(Debug, Clone, Serialize, Deserialize, clap::ValueEnum, PartialEq, Eq, Copy)]
#[serde(rename_all = "snake_case")]
pub enum TxPriority {
    NextBlock,
    HalfHour,
    OneHour,
}

impl TxPriority {
    pub fn n_blocks(&self) -> usize {
        match self {
            Self::NextBlock => 1,
            Self::HalfHour => 3,
            Self::OneHour => 6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq, Hash)]
pub struct Address(bitcoin::BdkAddress);

impl Address {
    pub fn script_pubkey(&self) -> bitcoin::ScriptBuf {
        self.0.script_pubkey()
    }

    pub fn parse_from_trusted_source(s: &str) -> Address {
        s.parse::<bitcoin::BdkAddress<_>>()
            .expect("should always parse address")
            .assume_checked()
            .into()
    }
}

impl From<bitcoin::BdkAddress> for Address {
    fn from(addr: bitcoin::BdkAddress) -> Self {
        Self(addr)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
impl std::str::FromStr for Address {
    type Err = <bitcoin::BdkAddress<bitcoin::NetworkUnchecked> as std::str::FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let address = bitcoin::BdkAddress::from_str(s)?.assume_checked();
        Ok(Address(address))
    }
}

impl TryFrom<(String, bitcoin::Network)> for Address {
    type Error = bitcoin::AddressError;
    fn try_from((address, network): (String, bitcoin::Network)) -> Result<Self, Self::Error> {
        let address = address
            .parse::<bitcoin::BdkAddress<_>>()?
            .require_network(network)?;
        Ok(Address(address))
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let address = s
            .parse::<bitcoin::BdkAddress<_>>()
            .map_err(|err| serde::de::Error::custom(err.to_string()))?
            .assume_checked();

        Ok(Address(address))
    }
}

pub type TxPayout = (uuid::Uuid, Address, Satoshis);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PayoutDestination {
    OnchainAddress { value: Address },
    Wallet { id: WalletId, address: Address },
}

impl PayoutDestination {
    pub fn onchain_address(&self) -> &Address {
        match self {
            Self::OnchainAddress { value } => value,
            Self::Wallet { address, .. } => address,
        }
    }
}

impl fmt::Display for PayoutDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        match self {
            PayoutDestination::OnchainAddress { value } => write!(f, "{value}"),
            PayoutDestination::Wallet { id, address } => write!(f, "wallet:{id}:{address}"),
        }
    }
}

pub const SATS_PER_BTC: Decimal = dec!(100_000_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Satoshis(Decimal);

impl fmt::Display for Satoshis {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for Satoshis {
    fn default() -> Self {
        Self::ZERO
    }
}

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

impl std::ops::Mul<usize> for Satoshis {
    type Output = Satoshis;
    fn mul(self, rhs: usize) -> Self {
        Satoshis::from(self.0 * Decimal::from(rhs))
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

impl std::ops::SubAssign<Satoshis> for Satoshis {
    fn sub_assign(&mut self, rhs: Satoshis) {
        *self = Self(self.0 - rhs.0)
    }
}

impl std::iter::Sum for Satoshis {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Satoshis::ZERO, |a, b| a + b)
    }
}

impl<'a> std::iter::Sum<&'a Satoshis> for Satoshis {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(Satoshis::ZERO, |a, b| a + *b)
    }
}
