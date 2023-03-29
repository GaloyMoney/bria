use serde::{Deserialize, Serialize};

use crate::primitives::bitcoin::Address;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PayoutDestination {
    OnchainAddress { value: Address },
}

impl PayoutDestination {
    pub fn onchain_address(&self) -> Option<Address> {
        match self {
            Self::OnchainAddress { value } => Some(value.clone()),
        }
    }
}
