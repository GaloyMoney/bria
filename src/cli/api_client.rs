use anyhow::Context;
use url::Url;

use crate::{
    api::proto,
    primitives::{bitcoin, TxPriority},
};
type ProtoClient = proto::bria_service_client::BriaServiceClient<tonic::transport::Channel>;

use super::token_store;

pub struct ApiClientConfig {
    pub url: Url,
}
impl Default for ApiClientConfig {
    fn default() -> Self {
        Self {
            url: Url::parse("http://localhost:2742").unwrap(),
        }
    }
}
pub struct ApiClient {
    config: ApiClientConfig,
    key: String,
    bria_home: String,
}

impl ApiClient {
    pub fn new(bria_home: String, config: ApiClientConfig, key: String) -> Self {
        Self {
            config,
            key,
            bria_home,
        }
    }

    async fn connect(&self) -> anyhow::Result<ProtoClient> {
        match ProtoClient::connect(self.config.url.to_string()).await {
            Ok(client) => Ok(client),
            Err(err) => {
                eprintln!(
                    "Couldn't connect to daemon\nAre you sure its running on {}?\n",
                    self.config.url
                );
                Err(anyhow::anyhow!(err))
            }
        }
    }

    pub fn inject_auth_token<T>(
        &self,
        mut request: tonic::Request<T>,
    ) -> anyhow::Result<tonic::Request<T>> {
        let key = if self.key.is_empty() {
            token_store::load_profile_token(&self.bria_home)?
        } else {
            self.key.clone()
        };

        request.metadata_mut().insert(
            crate::api::PROFILE_API_KEY_HEADER,
            tonic::metadata::MetadataValue::try_from(&key)
                .context("Couldn't create MetadataValue")?,
        );
        Ok(request)
    }

    pub async fn create_profile(
        &self,
        name: String,
        addresses: Option<Vec<String>>,
        max_payout: Option<u64>,
    ) -> anyhow::Result<()> {
        let policy = proto::SpendingPolicy {
            allowed_payout_addresses: addresses.unwrap_or_default(),
            max_payout_sats: max_payout,
        };
        let spending_policy =
            if policy.allowed_payout_addresses.is_empty() && policy.max_payout_sats.is_none() {
                None
            } else {
                Some(policy)
            };

        let request = tonic::Request::new(proto::CreateProfileRequest {
            name,
            spending_policy,
        });
        let response = self
            .connect()
            .await?
            .create_profile(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn update_profile(
        &self,
        id: String,
        addresses: Option<Vec<String>>,
        max_payout: Option<u64>,
    ) -> anyhow::Result<()> {
        let policy = proto::SpendingPolicy {
            allowed_payout_addresses: addresses.unwrap_or_default(),
            max_payout_sats: max_payout,
        };
        let spending_policy =
            if policy.allowed_payout_addresses.is_empty() && policy.max_payout_sats.is_none() {
                None
            } else {
                Some(policy)
            };

        let request = tonic::Request::new(proto::UpdateProfileRequest {
            id,
            spending_policy,
        });
        let response = self
            .connect()
            .await?
            .update_profile(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn list_profiles(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ListProfilesRequest {});
        let response = self
            .connect()
            .await?
            .list_profiles(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn create_profile_api_key(&self, profile_name: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::CreateProfileApiKeyRequest { profile_name });
        let response = self
            .connect()
            .await?
            .create_profile_api_key(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn import_xpub(
        &self,
        name: String,
        xpub: String,
        derivation: Option<String>,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ImportXpubRequest {
            name,
            xpub,
            derivation: derivation.unwrap_or_default(),
        });
        let response = self
            .connect()
            .await?
            .import_xpub(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn list_xpubs(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ListXpubsRequest {});
        let response = self
            .connect()
            .await?
            .list_xpubs(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn set_signer_config(
        &self,
        xpub_ref: String,
        config: impl TryInto<proto::set_signer_config_request::Config, Error = anyhow::Error>,
    ) -> anyhow::Result<()> {
        let config = config.try_into()?;
        let request = tonic::Request::new(proto::SetSignerConfigRequest {
            xpub_ref,
            config: Some(config),
        });
        let response = self
            .connect()
            .await?
            .set_signer_config(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn submit_signed_psbt(
        &self,
        batch_id: String,
        xpub_ref: String,
        signed_psbt: String,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::SubmitSignedPsbtRequest {
            batch_id,
            xpub_ref,
            signed_psbt,
        });
        let response = self
            .connect()
            .await?
            .submit_signed_psbt(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn create_wallet(
        &self,
        name: String,
        config: impl Into<proto::keychain_config::Config>,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::CreateWalletRequest {
            name,
            keychain_config: Some(proto::KeychainConfig {
                config: Some(config.into()),
            }),
        });
        let response = self
            .connect()
            .await?
            .create_wallet(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn get_wallet_balance_summary(&self, wallet_name: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::GetWalletBalanceSummaryRequest { wallet_name });
        let response = self
            .connect()
            .await?
            .get_wallet_balance_summary(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn get_account_balance_summary(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::GetAccountBalanceSummaryRequest {});
        let response = self
            .connect()
            .await?
            .get_account_balance_summary(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn new_address(
        &self,
        wallet: String,
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::NewAddressRequest {
            wallet_name: wallet,
            external_id,
            metadata: metadata.map(serde_json::from_value).transpose()?,
        });
        let response = self
            .connect()
            .await?
            .new_address(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn update_address(
        &self,
        address: String,
        new_external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::UpdateAddressRequest {
            address,
            new_external_id,
            new_metadata: metadata.map(serde_json::from_value).transpose()?,
        });
        let response = self
            .connect()
            .await?
            .update_address(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn list_addresses(&self, wallet: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ListAddressesRequest {
            wallet_name: wallet,
        });
        let response = self
            .connect()
            .await?
            .list_addresses(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn get_address(
        &self,
        address: Option<String>,
        external_id: Option<String>,
    ) -> anyhow::Result<()> {
        let identifier = match (address, external_id) {
            (Some(address), None) => proto::get_address_request::Identifier::Address(address),
            (None, Some(external_id)) => {
                proto::get_address_request::Identifier::ExternalId(external_id)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid parameters: you should provide either an address or an external_id"
                ));
            }
        };

        let request = tonic::Request::new(proto::GetAddressRequest {
            identifier: Some(identifier),
        });

        let response = self
            .connect()
            .await?
            .get_address(self.inject_auth_token(request)?)
            .await?;

        output_json(response)
    }

    pub async fn list_utxos(&self, wallet: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ListUtxosRequest {
            wallet_name: wallet,
        });
        let response = self
            .connect()
            .await?
            .list_utxos(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_payout_queue(
        &self,
        name: String,
        description: Option<String>,
        tx_priority: TxPriority,
        consolidate_deprecated_keychains: bool,
        interval_trigger: Option<u32>,
        manual_trigger: Option<bool>,
        cpfp_payouts_after_mins: Option<u32>,
        cpfp_payouts_after_blocks: Option<u32>,
        force_min_change_sats: Option<u64>,
    ) -> anyhow::Result<()> {
        let tx_priority = match tx_priority {
            TxPriority::NextBlock => proto::TxPriority::NextBlock as i32,
            TxPriority::HalfHour => proto::TxPriority::HalfHour as i32,
            TxPriority::OneHour => proto::TxPriority::OneHour as i32,
        };
        let trigger = match (interval_trigger, manual_trigger) {
            (Some(interval), None) | (Some(interval), Some(false)) => {
                Some(proto::payout_queue_config::Trigger::IntervalSecs(interval))
            }
            (None, Some(true)) => Some(proto::payout_queue_config::Trigger::Manual(true)),
            (Some(_), Some(true)) => {
                return Err(anyhow::anyhow!(
                    "Invalid parameters: you should provide either an interval_trigger or a manual_trigger"
                ));
            }
            _ => None,
        };

        let config = proto::PayoutQueueConfig {
            tx_priority,
            consolidate_deprecated_keychains,
            trigger,
            cpfp_payouts_after_mins,
            cpfp_payouts_after_blocks,
            force_min_change_sats,
        };

        let request = tonic::Request::new(proto::CreatePayoutQueueRequest {
            name,
            description,
            config: Some(config),
        });

        let response = self
            .connect()
            .await?
            .create_payout_queue(self.inject_auth_token(request)?)
            .await?;

        output_json(response)
    }

    pub async fn trigger_payout_queue(&self, name: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::TriggerPayoutQueueRequest { name });
        let response = self
            .connect()
            .await?
            .trigger_payout_queue(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn estimate_payout_fee(
        &self,
        wallet_name: String,
        payout_queue_name: String,
        destination: String,
        satoshis: u64,
    ) -> anyhow::Result<()> {
        let destination = if let Ok(addr) = destination.parse::<bitcoin::BdkAddress<_>>() {
            proto::estimate_payout_fee_request::Destination::OnchainAddress(
                addr.assume_checked().to_string(),
            )
        } else {
            proto::estimate_payout_fee_request::Destination::DestinationWalletName(destination)
        };
        let request = tonic::Request::new(proto::EstimatePayoutFeeRequest {
            wallet_name,
            payout_queue_name,
            destination: Some(destination),
            satoshis,
        });
        let response = self
            .connect()
            .await?
            .estimate_payout_fee(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn submit_payout(
        &self,
        wallet_name: String,
        payout_queue_name: String,
        destination: String,
        satoshis: u64,
        external_id: Option<String>,
        metadata: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        let destination = if let Ok(addr) = destination.parse::<bitcoin::BdkAddress<_>>() {
            proto::submit_payout_request::Destination::OnchainAddress(
                addr.assume_checked().to_string(),
            )
        } else {
            proto::submit_payout_request::Destination::DestinationWalletName(destination)
        };
        let request = tonic::Request::new(proto::SubmitPayoutRequest {
            wallet_name,
            payout_queue_name,
            destination: Some(destination),
            satoshis,
            external_id,
            metadata: metadata.map(serde_json::from_value).transpose()?,
        });
        let response = self
            .connect()
            .await?
            .submit_payout(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn list_wallets(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ListWalletsRequest {});
        let response = self
            .connect()
            .await?
            .list_wallets(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn list_payouts(
        &self,
        wallet: String,
        page: Option<u64>,
        page_size: Option<u64>,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ListPayoutsRequest {
            wallet_name: wallet,
            page,
            page_size,
        });
        let response = self
            .connect()
            .await?
            .list_payouts(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn get_payout(
        &self,
        id: Option<String>,
        external_id: Option<String>,
    ) -> anyhow::Result<()> {
        let identifier = match (id, external_id) {
            (Some(id), None) => proto::get_payout_request::Identifier::Id(id),
            (None, Some(external_id)) => {
                proto::get_payout_request::Identifier::ExternalId(external_id)
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid parameters: you should provide either a payout_id or an external_id"
                ));
            }
        };

        let request = tonic::Request::new(proto::GetPayoutRequest {
            identifier: Some(identifier),
        });

        let response = self
            .connect()
            .await?
            .get_payout(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn cancel_payout(&self, id: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::CancelPayoutRequest { id });
        let response = self
            .connect()
            .await?
            .cancel_payout(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn list_payout_queues(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ListPayoutQueuesRequest {});
        let response = self
            .connect()
            .await?
            .list_payout_queues(self.inject_auth_token(request)?)
            .await?;
        let result = response.into_inner();
        let payout_queues: Vec<_> = result
            .payout_queues
            .into_iter()
            .map(|bg| {
                let tx_priority = TxPriority::from(
                    proto::TxPriority::try_from(bg.config.as_ref().unwrap().tx_priority).unwrap(),
                );
                let mut json = serde_json::to_value(bg).unwrap();
                json.as_object_mut()
                    .unwrap()
                    .get_mut("config")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("txPriority".to_string(), format!("{tx_priority:?}").into());
                json
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "PayoutQueues": payout_queues,
            }))
            .unwrap()
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_payout_queue(
        &self,
        id: String,
        description: Option<String>,
        tx_priority: Option<TxPriority>,
        consolidate_deprecated_keychains: Option<bool>,
        interval_trigger: Option<u32>,
        cpfp_payouts_after_mins: Option<u32>,
        cpfp_payouts_after_blocks: Option<u32>,
        force_min_change_sats: Option<u64>,
    ) -> anyhow::Result<()> {
        let tx_priority = tx_priority.map(|priority| match priority {
            TxPriority::NextBlock => proto::TxPriority::NextBlock as i32,
            TxPriority::HalfHour => proto::TxPriority::HalfHour as i32,
            TxPriority::OneHour => proto::TxPriority::OneHour as i32,
        });

        let trigger = interval_trigger.map(proto::payout_queue_config::Trigger::IntervalSecs);

        let config = if let (Some(tx_priority), Some(consolidate_deprecated_keychains)) =
            (tx_priority, consolidate_deprecated_keychains)
        {
            Some(proto::PayoutQueueConfig {
                tx_priority,
                consolidate_deprecated_keychains,
                trigger,
                cpfp_payouts_after_mins,
                cpfp_payouts_after_blocks,
                force_min_change_sats,
            })
        } else {
            None
        };
        let request = tonic::Request::new(proto::UpdatePayoutQueueRequest {
            id,
            new_description: description,
            new_config: config,
        });
        let response = self
            .connect()
            .await?
            .update_payout_queue(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn get_batch(&self, id: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::GetBatchRequest { id });
        let response = self
            .connect()
            .await?
            .get_batch(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn cancel_batch(&self, id: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::CancelBatchRequest { id });
        let response = self
            .connect()
            .await?
            .cancel_batch(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn watch_events(
        &self,
        one_shot: bool,
        after_sequence: Option<u64>,
        augment: bool,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::SubscribeAllRequest {
            after_sequence,
            augment: Some(augment),
        });

        let mut stream = self
            .connect()
            .await?
            .subscribe_all(self.inject_auth_token(request)?)
            .await?
            .into_inner();

        while let Some(event) = stream.message().await? {
            println!("{}", serde_json::to_string_pretty(&event)?);
            if one_shot {
                break;
            }
        }

        Ok(())
    }
}

fn output_json<T: serde::Serialize>(response: tonic::Response<T>) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&response.into_inner())?);
    Ok(())
}
