use anyhow::Context;
use url::Url;

use crate::{api::proto, primitives::TxPriority};
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

    pub async fn create_profile(&self, name: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::CreateProfileRequest { name });
        let response = self
            .connect()
            .await?
            .create_profile(self.inject_auth_token(request)?)
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

    pub async fn set_signer_config(
        &self,
        xpub_ref: String,
        config: impl Into<proto::LndSignerConfig>,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::SetSignerConfigRequest {
            xpub_ref,
            config: Some(proto::set_signer_config_request::Config::Lnd(config.into())),
        });
        let response = self
            .connect()
            .await?
            .set_signer_config(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn create_wallet(&self, name: String, xpubs: Vec<String>) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::CreateWalletRequest {
            name,
            xpub_refs: xpubs,
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

    pub async fn new_address(&self, wallet: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::NewAddressRequest {
            wallet_name: wallet,
        });
        let response = self
            .connect()
            .await?
            .new_address(self.inject_auth_token(request)?)
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
    pub async fn create_batch_group(
        &self,
        name: String,
        description: Option<String>,
        tx_priority: TxPriority,
        consolidate_deprecated_keychains: bool,
        manual_trigger: bool,
        immediate_trigger: bool,
        interval_trigger: Option<u32>,
    ) -> anyhow::Result<()> {
        let tx_priority = match tx_priority {
            TxPriority::NextBlock => proto::TxPriority::NextBlock as i32,
            TxPriority::OneHour => proto::TxPriority::OneHour as i32,
            TxPriority::Economy => proto::TxPriority::Economy as i32,
        };

        let trigger = if manual_trigger {
            Some(proto::batch_group_config::Trigger::Manual(manual_trigger))
        } else if immediate_trigger {
            Some(proto::batch_group_config::Trigger::Immediate(
                immediate_trigger,
            ))
        } else {
            interval_trigger.map(proto::batch_group_config::Trigger::IntervalSecs)
        };

        let config = proto::BatchGroupConfig {
            tx_priority,
            consolidate_deprecated_keychains,
            trigger,
        };

        let request = tonic::Request::new(proto::CreateBatchGroupRequest {
            name,
            description,
            config: Some(config),
        });

        let response = self
            .connect()
            .await?
            .create_batch_group(self.inject_auth_token(request)?)
            .await?;

        output_json(response)
    }

    pub async fn queue_payout(
        &self,
        wallet_name: String,
        batch_group_name: String,
        on_chain_address: String,
        satoshis: u64,
        metadata: serde_json::Value,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::QueuePayoutRequest {
            wallet_name,
            batch_group_name,
            destination: Some(proto::queue_payout_request::Destination::OnchainAddress(
                on_chain_address,
            )),
            satoshis,
            metadata: Some(serde_json::from_value(metadata).unwrap()),
        });
        let response = self
            .connect()
            .await?
            .queue_payout(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }

    pub async fn list_payouts(&self, wallet: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ListPayoutsRequest {
            wallet_name: wallet,
        });
        let response = self
            .connect()
            .await?
            .list_payouts(self.inject_auth_token(request)?)
            .await?;
        output_json(response)
    }
}

fn output_json<T: serde::Serialize>(response: tonic::Response<T>) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&response.into_inner())?);
    Ok(())
}
