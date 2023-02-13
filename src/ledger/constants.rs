use uuid::{uuid, Uuid};

// Transaction Template Codes
pub(super) const INCOMING_UTXO_CODE: &str = "INCOMING_UTXO";
pub(super) const CONFIRMED_UTXO_CODE: &str = "CONFIRMED_UTXO";
pub(super) const QUEUED_PAYOUT_CODE: &str = "QUEUED_PAYOUT";
pub(super) const CREATE_BATCH_CODE: &str = "CREATE_BATCH";
pub(super) const CONFIRMED_UTXO_WITHOUT_FEE_RESERVE_CODE: &str = "CONFIRMED_UTXO_WO_FR";

// Onchain/Omnibus Ledger Accounts
pub(super) const ONCHAIN_INCOMING_CODE: &str = "ONCHAIN_INCOMING";
pub(super) const ONCHAIN_AT_REST_CODE: &str = "ONCHAIN_AT_REST";
pub(super) const ONCHAIN_FEE_CODE: &str = "ONCHAIN_FEE";
pub(super) const ONCHAIN_OUTGOING_CODE: &str = "ONCHAIN_OUTGOING";

pub(super) const ONCHAIN_INCOMING_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
pub(super) const ONCHAIN_AT_REST_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");
pub(super) const ONCHAIN_FEE_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000003");
pub(super) const ONCHAIN_OUTGOING_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000004");
