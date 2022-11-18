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
}

impl AdminApiClient {
    pub fn new(config: AdminApiClientConfig) -> Self {
        Self { config }
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
    pub async fn bootstrap(&self) -> anyhow::Result<()> {
        let request = tonic::Request::new(proto::BootstrapRequest {});
        let response = self.connect().await?.bootstrap(request).await?;
        print_admin_api_key(response.into_inner().key.context("No key returned")?);
        Ok(())
    }
}

pub fn print_admin_api_key(key: proto::AdminApiKey) {
    println!("Admin API key");
    println!("---\nname: {}\nkey: {}\nid: {}", key.name, key.key, key.id,);
}
