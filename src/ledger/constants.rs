use uuid::{uuid, Uuid};

pub(super) const ONCHAIN_INCOME_CODE: &str = "ONCHAIN_INCOME";
pub(super) const ONCHAIN_INCOMING_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
pub(super) const INCOMING_UTXO_CODE: &str = "INCOMING_UTXO";
pub(super) const CONFIRMED_UTXO_CODE: &str = "CONFIRMED_UTXO";
