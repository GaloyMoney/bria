use anyhow::Context;
use url::Url;

use crate::admin::proto;
type ProtoClient = proto::admin_service_client::AdminServiceClient<tonic::transport::Channel>;

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
}

impl AdminApiClient {
    pub fn new(config: AdminApiClientConfig, key: String) -> Self {
        println!("API KEY: {}", key);
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
        if self.key.is_empty() {
            return Err(anyhow::anyhow!("No BRIA_ADMIN_API_KEY specified"));
        }

        request.metadata_mut().insert(
            crate::admin::ADMIN_API_KEY_HEADER,
            tonic::metadata::MetadataValue::try_from(&self.key)
                .context("Couldn't create MetadataValue")?,
        );
        Ok(request)
    }

    pub async fn bootstrap(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::BootstrapRequest {});
        let response = self.connect().await?.bootstrap(request).await?;
        print_admin_api_key(response.into_inner().key.context("No key returned")?);
        Ok(())
    }

    pub async fn account_create(&self, name: String) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::AccountCreateRequest { name });
        let response = self
            .connect()
            .await?
            .account_create(self.inject_auth_token(request)?)
            .await?;
        print_account(response.into_inner().key.context("No key returned")?);
        Ok(())
    }
}

pub fn print_admin_api_key(key: proto::AdminApiKey) {
    println!("Admin API key");
    println!("---\nname: {}\nkey: {}\nid: {}", key.name, key.key, key.id,);
}

pub fn print_account(key: proto::AccountApiKey) {
    println!("New Account");
    println!(
        "---\nname: {}\nid: {}\nkey: {}\nkey_id: {}",
        key.name, key.account_id, key.key, key.id,
    );
}
