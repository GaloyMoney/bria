use uuid::{uuid, Uuid};

// Transaction Template Codes
pub(super) const UTXO_DETECTED_CODE: &str = "UTXO_DETECTED";
pub(super) const UTXO_DETECTED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");

pub(super) const UTXO_SETTLED_CODE: &str = "UTXO_SETTLED";
pub(super) const UTXO_SETTLED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");

pub(super) const SPENT_UTXO_SETTLED_CODE: &str = "SPENT_UTXO_SETTLED";
pub(super) const SPENT_UTXO_SETTLED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000003");

pub(super) const SPEND_DETECTED_CODE: &str = "SPEND_DETECTED";
pub(super) const SPEND_DETECTED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000004");

pub(super) const SPEND_SETTLED_CODE: &str = "SPEND_SETTLED";
pub(super) const SPEND_SETTLED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000005");

pub(super) const PAYOUT_QUEUED_CODE: &str = "PAYOUT_QUEUED";
pub(super) const PAYOUT_QUEUED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000006");

pub(super) const BATCH_CREATED_CODE: &str = "BATCH_CREATED";
pub(super) const BATCH_CREATED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000007");

pub(super) const BATCH_SUBMITTED_CODE: &str = "BATCH_SUBMITTED";
pub(super) const BATCH_SUBMITTED_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000008");

// Onchain/Omnibus Ledger Accounts
pub(super) const ONCHAIN_UTXO_INCOMING_CODE: &str = "ONCHAIN_UTXO_INCOMING";
pub(super) const ONCHAIN_UTXO_INCOMING_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");

pub(super) const ONCHAIN_UTXO_AT_REST_CODE: &str = "ONCHAIN_UTXO_AT_REST";
pub(super) const ONCHAIN_UTXO_AT_REST_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");

pub(super) const ONCHAIN_UTXO_OUTGOING_CODE: &str = "ONCHAIN_UTXO_OUTGOING";
pub(super) const ONCHAIN_UTXO_OUTGOING_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000004");

pub(super) const ONCHAIN_FEE_CODE: &str = "ONCHAIN_FEE";
pub(super) const ONCHAIN_FEE_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000003");

pub(super) const LOGICAL_INCOMING_CODE: &str = "LOGICAL_INCOMING";
pub(super) const LOGICAL_INCOMING_ID: Uuid = uuid!("10000000-0000-0000-0000-000000000001");

pub(super) const LOGICAL_AT_REST_CODE: &str = "LOGICAL_AT_REST";
pub(super) const LOGICAL_AT_REST_ID: Uuid = uuid!("10000000-0000-0000-0000-000000000002");

pub(super) const LOGICAL_OUTGOING_CODE: &str = "LOGICAL_OUTGOING";
pub(super) const LOGICAL_OUTGOING_ID: Uuid = uuid!("10000000-0000-0000-0000-000000000003");