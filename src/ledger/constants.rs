use uuid::{uuid, Uuid};

// Transaction Template Codes
pub(super) const UTXO_DETECTED_CODE: &str = "UTXO_DETECTED";
pub(super) const UTXO_DETECTED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");

pub(super) const UTXO_SETTLED_CODE: &str = "UTXO_SETTLED";
pub(super) const UTXO_SETTLED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");

pub(super) const UTXO_DROPPED_CODE: &str = "UTXO_DROPPED";
pub(super) const UTXO_DROPPED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000009");

pub(super) const SPENT_UTXO_SETTLED_CODE: &str = "SPENT_UTXO_SETTLED";
pub(super) const SPENT_UTXO_SETTLED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000003");

pub(super) const SPEND_DETECTED_CODE: &str = "SPEND_DETECTED";
pub(super) const SPEND_DETECTED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000004");

pub(super) const SPEND_SETTLED_CODE: &str = "SPEND_SETTLED";
pub(super) const SPEND_SETTLED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000005");

pub(super) const PAYOUT_SUBMITTED_CODE: &str = "PAYOUT_SUBMITTED";
pub(super) const PAYOUT_SUBMITTED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000006");

pub(super) const PAYOUT_CANCELLED_CODE: &str = "PAYOUT_CANCELLED";
pub(super) const PAYOUT_CANCELLED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000010");

pub(super) const _BATCH_CREATED_LEGACY_CODE: &str = "BATCH_CREATED";
pub(super) const _BATCH_CREATED_LEGACY_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000007");

pub(super) const FIX_BATCH_CREATED_LEGACY_CODE: &str = "FIX_BATCH_CREATED";
pub(super) const FIX_BATCH_CREATED_LEGACY_ID: Uuid = uuid!("00000000-0000-0000-0000-100000000007");

pub(super) const BATCH_CREATED_CODE: &str = "BATCH_CREATED_V2";
pub(super) const BATCH_CREATED_ID: Uuid = uuid!("10000000-0000-0000-0000-000000000007");

pub(super) const BATCH_BROADCAST_CODE: &str = "BATCH_BROADCAST";
pub(super) const BATCH_BROADCAST_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000008");

pub(super) const BATCH_DROPPED_CODE: &str = "BATCH_DROPPED";
pub(super) const BATCH_DROPPED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000011");

// Onchain/Omnibus Ledger Accounts
pub(super) const ONCHAIN_UTXO_INCOMING_CODE: &str = "ONCHAIN_UTXO_INCOMING";
pub(super) const ONCHAIN_UTXO_INCOMING_ID: Uuid = uuid!("00000000-1910-0000-1000-000000000000");

pub(super) const ONCHAIN_UTXO_AT_REST_CODE: &str = "ONCHAIN_UTXO_AT_REST";
pub(super) const ONCHAIN_UTXO_AT_REST_ID: Uuid = uuid!("00000000-1900-0000-1000-000000000000");

pub(super) const ONCHAIN_UTXO_OUTGOING_CODE: &str = "ONCHAIN_UTXO_OUTGOING";
pub(super) const ONCHAIN_UTXO_OUTGOING_ID: Uuid = uuid!("00000000-1920-0000-1000-000000000000");

pub(super) const ONCHAIN_FEE_CODE: &str = "ONCHAIN_FEE";
pub(super) const ONCHAIN_FEE_ID: Uuid = uuid!("00000000-6900-0000-3000-000000000000");

pub(super) const EFFECTIVE_INCOMING_CODE: &str = "EFFECTIVE_INCOMING";
pub(super) const EFFECTIVE_INCOMING_ID: Uuid = uuid!("00000000-1910-0000-2000-000000000000");

pub(super) const EFFECTIVE_AT_REST_CODE: &str = "EFFECTIVE_AT_REST";
pub(super) const EFFECTIVE_AT_REST_ID: Uuid = uuid!("00000000-1900-0000-2000-000000000000");

pub(super) const EFFECTIVE_OUTGOING_CODE: &str = "EFFECTIVE_OUTGOING";
pub(super) const EFFECTIVE_OUTGOING_ID: Uuid = uuid!("00000000-1920-0000-2000-000000000000");

pub const CURRENCY_CODE: &str = "00000000";
pub enum Element {
    #[allow(dead_code)] // Used in omnibus accounts
    Asset,
    Liability,
    Revenue,
    #[allow(dead_code)] // Used in omnibus accounts
    Expense,
}

impl Element {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Asset => "1",
            Self::Liability => "2",
            Self::Revenue => "4",
            Self::Expense => "6",
        }
    }
}

pub const HOT_WALLET_CODE: &str = "0";

pub enum SubGroup {
    AtRest,
    Incoming,
    Outgoing,
}

impl SubGroup {
    pub fn code(&self) -> &'static str {
        match self {
            SubGroup::AtRest => "00",
            SubGroup::Incoming => "10",
            SubGroup::Outgoing => "20",
        }
    }
}

pub const RESERVED: &str = "0000";

pub enum Category {
    Onchain,
    Effective,
    Fee,
    Dust,
}

impl Category {
    pub fn code(&self) -> &'static str {
        match self {
            Category::Onchain => "1000",
            Category::Effective => "2000",
            Category::Fee => "3000",
            Category::Dust => "0000",
        }
    }
}
