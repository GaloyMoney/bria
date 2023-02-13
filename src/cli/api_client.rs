use anyhow::Context;
use url::Url;

use crate::api::proto;
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
}

impl ApiClient {
    pub fn new(config: ApiClientConfig, key: String) -> Self {
        Self { config, key }
    }

    async fn connect(&self) -> anyhow::Result<ProtoClient> {
        match ProtoClient::connect(self.config.url.to_string()).await {
            Ok(client) => Ok(client),
            Err(err) => {
                eprintln!(
                    "Couldn't connect to price server\nAre you sure its running on {}?\n",
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
            token_store::load_account_token()?
        } else {
            self.key.clone()
        };

        request.metadata_mut().insert(
            crate::api::ACCOUNT_API_KEY_HEADER,
            tonic::metadata::MetadataValue::try_from(&key)
                .context("Couldn't create MetadataValue")?,
        );
        Ok(request)
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
        println!("XPUB imported - {}", response.into_inner().id);
        Ok(())
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
        self.connect()
            .await?
            .set_signer_config(self.inject_auth_token(request)?)
            .await?;
        println!("Done");
        Ok(())
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
        println!("Wallet created - {}", response.into_inner().id);
        Ok(())
    }

    pub async fn get_wallet_balance_summary(
        &self,
        wallet_name: String,
        json: bool,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::GetWalletBalanceSummaryRequest { wallet_name });
        let response = self
            .connect()
            .await?
            .get_wallet_balance_summary(self.inject_auth_token(request)?)
            .await?;
        if json {
            println!("{}", serde_json::to_string_pretty(&response.into_inner())?);
        } else {
            let proto::GetWalletBalanceSummaryResponse {
                current_settled,
                pending_incoming,
                pending_outgoing,
                pending_fees,
                encumbered_fees,
                encumbered_outgoing,
            } = response.into_inner();

            println!("Pending Incoming: {pending_incoming}");
            println!("Current Settled: {current_settled}");
            println!("Encumbered Fees: {encumbered_fees}");
            println!("Encumbered Outgoing: {encumbered_outgoing}");
            println!("Pending Outgoing: {pending_outgoing}");
            println!("Pending Fees: {pending_fees}");
        }

        Ok(())
    }

    pub async fn new_address(&self, wallet: String, raw: bool) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::NewAddressRequest {
            wallet_name: wallet,
        });
        let response = self
            .connect()
            .await?
            .new_address(self.inject_auth_token(request)?)
            .await?;
        if raw {
            print!("{}", response.into_inner().address);
        } else {
            println!("New Address - {}", response.into_inner().address);
        }
        Ok(())
    }

    pub async fn create_batch_group(&self, name: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::CreateBatchGroupRequest { name });
        let response = self
            .connect()
            .await?
            .create_batch_group(self.inject_auth_token(request)?)
            .await?;
        println!("BatchGroup created - {}", response.into_inner().id);
        Ok(())
    }

    pub async fn queue_payout(
        &self,
        wallet_name: String,
        batch_group_name: String,
        on_chain_address: String,
        satoshis: u64,
    ) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::QueuePayoutRequest {
            wallet_name,
            batch_group_name,
            destination: Some(proto::queue_payout_request::Destination::OnchainAddress(
                on_chain_address,
            )),
            satoshis,
        });
        let response = self
            .connect()
            .await?
            .queue_payout(self.inject_auth_token(request)?)
            .await?;
        println!("Payout enqueued - {}", response.into_inner().id);
        Ok(())
    }
}
