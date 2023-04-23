use uuid::{uuid, Uuid};

// Transaction Template Codes
pub(super) const INCOMING_UTXO_CODE: &str = "INCOMING_UTXO";
pub(super) const INCOMING_UTXO_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");

pub(super) const CONFIRMED_UTXO_CODE: &str = "CONFIRMED_UTXO";
pub(super) const CONFIRMED_UTXO_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");

pub(super) const CONFIRM_SPENT_UTXO_CODE: &str = "CONFIRM_SPENT_UTXO";
pub(super) const CONFIRM_SPENT_UTXO_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000003");

pub(super) const EXTERNAL_SPEND_CODE: &str = "EXTERNAL_SPEND";
pub(super) const EXTERNAL_SPEND_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000004");

pub(super) const CONFIRM_SPEND_CODE: &str = "CONFIRM_SPEND";
pub(super) const CONFIRM_SPEND_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000005");

pub(super) const QUEUED_PAYOUT_CODE: &str = "QUEUED_PAYOUT";
pub(super) const QUEUED_PAYOUD_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000006");

pub(super) const CREATE_BATCH_CODE: &str = "CREATE_BATCH";
pub(super) const CREATE_BATCH_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000007");

pub(super) const SUBMIT_BATCH_CODE: &str = "SUBMIT_BATCH";
pub(super) const SUBMIT_BATCH_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000008");

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
