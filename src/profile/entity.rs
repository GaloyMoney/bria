use derive_builder::Builder;
use es_entity::*;
use serde::{Deserialize, Serialize};

use crate::primitives::*;

#[derive(EsEvent, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[es_event(id = "ProfileId")]
pub enum ProfileEvent {
    Initialized {
        id: ProfileId,
        account_id: AccountId,
    },
    NameUpdated {
        name: String,
    },
    SpendingPolicyUpdated {
        spending_policy: SpendingPolicy,
    },
    SpendingPolicyRemoved {},
}

#[derive(EsEntity, Builder)]
#[builder(pattern = "owned", build_fn(error = "EsEntityError"))]
pub struct Profile {
    pub id: ProfileId,
    pub account_id: AccountId,
    pub name: String,
    #[builder(default)]
    pub spending_policy: Option<SpendingPolicy>,
    pub(super) events: EntityEvents<ProfileEvent>,
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Profile")
            .field("id", &self.id)
            .field("account_id", &self.account_id)
            .field("name", &self.name)
            .field("spending_policy", &self.spending_policy)
            .finish()
    }
}

impl Profile {
    pub fn update_spending_policy(&mut self, policy: Option<SpendingPolicy>) {
        if self.spending_policy != policy {
            self.spending_policy.clone_from(&policy);
            if let Some(policy) = policy {
                self.events.push(ProfileEvent::SpendingPolicyUpdated {
                    spending_policy: policy,
                });
            } else {
                self.events.push(ProfileEvent::SpendingPolicyRemoved {});
            }
        }
    }

    pub fn is_destination_allowed(&self, destination: &PayoutDestination) -> bool {
        self.spending_policy
            .as_ref()
            .map(|sp| sp.is_destination_allowed(destination))
            .unwrap_or(true)
    }

    pub fn is_amount_allowed(&self, sats: Satoshis) -> bool {
        self.spending_policy
            .as_ref()
            .map(|sp| sp.is_amount_allowed(sats))
            .unwrap_or(true)
    }
}

impl TryFromEvents<ProfileEvent> for Profile {
    fn try_from_events(events: EntityEvents<ProfileEvent>) -> Result<Self, EsEntityError> {
        let mut builder = ProfileBuilder::default();
        for event in events.iter_all() {
            match event {
                ProfileEvent::Initialized { id, account_id } => {
                    builder = builder.id(*id).account_id(*account_id);
                }
                ProfileEvent::NameUpdated { name } => {
                    builder = builder.name(name.clone());
                }
                ProfileEvent::SpendingPolicyUpdated { spending_policy } => {
                    builder = builder.spending_policy(Some(spending_policy.clone()));
                }
                ProfileEvent::SpendingPolicyRemoved {} => builder = builder.spending_policy(None),
            }
        }
        builder.events(events).build()
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpendingPolicy {
    pub allowed_payout_addresses: Vec<Address>,
    pub max_payout: Option<Satoshis>,
}

impl SpendingPolicy {
    fn is_destination_allowed(&self, destination: &PayoutDestination) -> bool {
        self.allowed_payout_addresses.is_empty()
            || self
                .allowed_payout_addresses
                .contains(destination.onchain_address())
    }

    fn is_amount_allowed(&self, amount: Satoshis) -> bool {
        self.max_payout.map(|max| amount <= max).unwrap_or(true)
    }
}

pub struct ProfileApiKey {
    pub key: String,
    pub id: ProfileApiKeyId,
    pub profile_id: ProfileId,
    pub account_id: AccountId,
}

#[derive(Builder, Clone, Debug)]
pub struct NewProfile {
    #[builder(setter(into))]
    pub(super) id: ProfileId,
    #[builder(setter(into))]
    pub(super) account_id: AccountId,
    #[builder(setter(into))]
    pub(super) name: String,
    #[builder(default)]
    pub(super) spending_policy: Option<SpendingPolicy>,
}

impl NewProfile {
    pub fn builder() -> NewProfileBuilder {
        let mut builder = NewProfileBuilder::default();
        builder.id(ProfileId::new());
        builder
    }
}

impl IntoEvents<ProfileEvent> for NewProfile {
    fn into_events(self) -> EntityEvents<ProfileEvent> {
        let mut events = vec![
            ProfileEvent::Initialized {
                id: self.id,
                account_id: self.account_id,
            },
            ProfileEvent::NameUpdated { name: self.name },
        ];
        if let Some(spending_policy) = self.spending_policy {
            events.push(ProfileEvent::SpendingPolicyUpdated { spending_policy });
        }
        EntityEvents::init(self.id, events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allow_all_addresses_if_allowed_list_empty() {
        let address = Address::parse_from_trusted_source("mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU");
        let policy = super::SpendingPolicy {
            allowed_payout_addresses: vec![],
            max_payout: Some(Satoshis::from(1000)),
        };
        assert!(
            policy.is_destination_allowed(&PayoutDestination::OnchainAddress { value: address })
        );
    }

    #[test]
    fn block_address_not_in_allowed_list() {
        let address = Address::parse_from_trusted_source("mgWUuj1J1N882jmqFxtDepEC73Rr22E9GU");
        let policy = super::SpendingPolicy {
            allowed_payout_addresses: vec![Address::parse_from_trusted_source(
                "bcrt1q4gfcga7jfjmm02zpvrh4ttc5k7lmnq2re52z2y",
            )],
            max_payout: Some(Satoshis::from(1000)),
        };

        assert!(
            !policy.is_destination_allowed(&PayoutDestination::OnchainAddress { value: address })
        );
    }

    #[test]
    fn allow_address_in_allowed_list() {
        let address =
            Address::parse_from_trusted_source("bcrt1q4gfcga7jfjmm02zpvrh4ttc5k7lmnq2re52z2y");
        let policy = super::SpendingPolicy {
            allowed_payout_addresses: vec![address.clone()],
            max_payout: Some(Satoshis::from(1000)),
        };

        assert!(
            policy.is_destination_allowed(&PayoutDestination::OnchainAddress { value: address })
        );
    }
}
