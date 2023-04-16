mod convert;

#[allow(clippy::all)]
pub mod proto {
    tonic::include_proto!("services.bria_admin.v1");
}

use tonic::{transport::Server, Request, Response, Status};
use tracing::instrument;

use proto::{admin_service_server::AdminService, *};

use super::{app::*, config::*, error::*};

pub const ADMIN_API_KEY_HEADER: &str = "x-bria-admin-api-key";

pub struct Admin {
    app: AdminApp,
}

#[tonic::async_trait]
impl AdminService for Admin {
    #[instrument(skip_all, err)]
    async fn bootstrap(
        &self,
        _request: Request<BootstrapRequest>,
    ) -> Result<Response<BootstrapResponse>, Status> {
        let super::AdminApiKey { id, name, key } = self.app.bootstrap().await?;
        Ok(Response::new(BootstrapResponse {
            key: Some(AdminApiKey {
                id: id.to_string(),
                name,
                key,
            }),
        }))
    }

    #[instrument(skip_all, err)]
    async fn create_account(
        &self,
        request: Request<CreateAccountRequest>,
    ) -> Result<Response<CreateAccountResponse>, Status> {
        let admin_api_key = extract_api_token(&request)?;
        self.app.authenticate(admin_api_key).await?;
        let name = request.into_inner().name;
        let key = self.app.create_account(name.clone()).await?;
        Ok(Response::new(CreateAccountResponse {
            key: Some(ProfileApiKey {
                profile_id: key.profile_id.to_string(),
                name,
                key: key.key,
                account_id: key.account_id.to_string(),
            }),
        }))
    }

    #[instrument(skip_all, err)]
    async fn list_accounts(
        &self,
        request: Request<ListAccountsRequest>,
    ) -> Result<Response<ListAccountsResponse>, Status> {
        let admin_api_key = extract_api_token(&request)?;
        self.app.authenticate(admin_api_key).await?;
        let accounts = self.app.list_accounts().await?;
        let response_accounts = accounts
            .into_iter()
            .map(|account| Account {
                id: account.id.to_string(),
                name: account.name,
            })
            .collect();
        Ok(Response::new(ListAccountsResponse {
            accounts: response_accounts,
        }))
    }
}

pub(crate) async fn start(
    server_config: AdminApiConfig,
    app: AdminApp,
) -> Result<(), AdminApiError> {
    let price_service = Admin { app };
    println!(
        "Starting admin server on port {}",
        server_config.listen_port
    );
    Server::builder()
        .add_service(proto::admin_service_server::AdminServiceServer::new(
            price_service,
        ))
        .serve(([0, 0, 0, 0], server_config.listen_port).into())
        .await?;
    Ok(())
}

pub fn extract_api_token<T>(request: &Request<T>) -> Result<&str, Status> {
    match request.metadata().get(ADMIN_API_KEY_HEADER) {
        Some(value) => value
            .to_str()
            .map_err(|_| Status::unauthenticated("Bad token")),
        None => Err(Status::unauthenticated(format!(
            "{ADMIN_API_KEY_HEADER} missing"
        ))),
    }
}
