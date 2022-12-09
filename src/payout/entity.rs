use derive_builder::Builder;

use crate::primitives::*;

use super::destination::*;

pub struct Payout {
    pub id: PayoutId,
    pub wallet_id: WalletId,
    pub destination: PayoutDestination,
    pub satoshis: u64,
}

#[derive(Debug, Builder, Clone)]
pub struct NewPayout {
    #[builder(setter(into))]
    pub(super) id: PayoutId,
    #[builder(setter(into))]
    pub(super) wallet_id: WalletId,
    #[builder(setter(into))]
    pub(super) batch_group_id: BatchGroupId,
    pub(super) satoshis: u64,
    pub(super) destination: PayoutDestination,
    #[builder(setter(into))]
    pub(super) external_id: String,
    #[builder(default, setter(into))]
    pub(super) metadata: Option<serde_json::Value>,
}

impl NewPayout {
    pub fn builder() -> NewPayoutBuilder {
        let mut builder = NewPayoutBuilder::default();
        let id = PayoutId::new();
        builder.external_id(id.to_string()).id(id);
        builder
    }
}
