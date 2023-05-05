use anyhow::Context;
use url::Url;

use crate::admin::proto;
type ProtoClient = proto::admin_service_client::AdminServiceClient<tonic::transport::Channel>;

use super::token_store;

pub struct AdminApiClientConfig {
    pub url: Url,
}
impl Default for AdminApiClientConfig {
    fn default() -> Self {
        Self {
            url: Url::parse("http://localhost:2743").unwrap(),
        }
    }
}
pub struct AdminApiClient {
    config: AdminApiClientConfig,
    key: String,
    bria_home: String,
}

impl AdminApiClient {
    pub fn new(bria_home: String, config: AdminApiClientConfig, key: String) -> Self {
        Self {
            bria_home,
            config,
            key,
        }
    }

    async fn connect(&self) -> anyhow::Result<ProtoClient> {
        match ProtoClient::connect(self.config.url.to_string()).await {
            Ok(client) => Ok(client),
            Err(err) => {
                eprintln!(
                    "Couldn't connect to bria admin server\nAre you sure its running on {}?\n",
                    self.config.url
                );
                Err(anyhow::anyhow!(err))
            }
        }
    }

    pub fn inject_admin_auth_token<T>(
        &self,
        mut request: tonic::Request<T>,
    ) -> anyhow::Result<tonic::Request<T>> {
        let key = if self.key.is_empty() {
            token_store::load_admin_token(&self.bria_home)?
        } else {
            self.key.clone()
        };

        request.metadata_mut().insert(
            crate::admin::ADMIN_API_KEY_HEADER,
            tonic::metadata::MetadataValue::try_from(&key)
                .context("Couldn't create MetadataValue")?,
        );
        Ok(request)
    }

    pub async fn bootstrap(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::BootstrapRequest {});
        let response = self.connect().await?.bootstrap(request).await?;
        let key = response.into_inner().key.context("No key in response")?;
        token_store::store_admin_token(&self.bria_home, &key.key)?;
        print_admin_api_key(key);
        Ok(())
    }

    pub async fn dev_bootstrap(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::DevBootstrapRequest {});
        let response = self.connect().await?.dev_bootstrap(request).await?;
        let key = response.into_inner().key.context("No key in response")?;
        token_store::store_admin_token(&self.bria_home, &key.key)?;
        print_admin_api_key(key);
        Ok(())
    }

    pub async fn account_create(&self, name: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::CreateAccountRequest { name });
        let response = self
            .connect()
            .await?
            .create_account(self.inject_admin_auth_token(request)?)
            .await?;
        let key = response.into_inner().key.context("No key in response")?;
        token_store::store_profile_token(&self.bria_home, &key.key)?;
        print_account(key);
        Ok(())
    }

    pub async fn list_accounts(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::ListAccountsRequest {});
        let response = self
            .connect()
            .await?
            .list_accounts(self.inject_admin_auth_token(request)?)
            .await?;
        output_json(response)
    }
}

pub fn print_admin_api_key(key: proto::AdminApiKey) {
    println!("Admin API key");
    println!("---\nname: {}\nkey: {}\nid: {}", key.name, key.key, key.id,);
}

pub fn print_account(key: proto::ProfileApiKey) {
    println!("New Account");
    println!(
        "---\nname: {}\nid: {}\nkey: {}\nprofile_id: {}",
        key.name, key.account_id, key.key, key.profile_id,
    );
}

fn output_json<T: serde::Serialize>(response: tonic::Response<T>) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&response.into_inner())?);
    Ok(())
}
