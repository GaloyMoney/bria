mod convert;

#[allow(clippy::all)]
pub mod proto {
    tonic::include_proto!("services.admin.v1");
}

use tonic::{transport::Server, Request, Response, Status};
use tracing::instrument;

use proto::{admin_service_server::AdminService, *};

use super::{app::*, config::*, error::*};

const ADMIN_API_KEY_HEADER: &str = "X_BRIA_ADMIN_API_KEY";

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
    async fn account_create(
        &self,
        request: Request<AccountCreateRequest>,
    ) -> Result<Response<AccountCreateResponse>, Status> {
        let admin_api_key = extract_api_token(&request)?;
        self.app.authenticate(admin_api_key).await?;
        self.app.account_create(request.into_inner().name).await?;
        unimplemented!()
        // let super::AdminApiKey { id, name, key } = self.app.bootstrap().await?;
        // Ok(Response::new(BootstrapResponse {
        //     key: Some(AdminApiKey {
        //         id: id.to_string(),
        //         name,
        //         key,
        //     }),
        // }))
    }
}

pub(crate) async fn start(
    server_config: AdminApiConfig,
    app: AdminApp,
) -> Result<(), AdminApiError> {
    let price_service = Admin { app };
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
            "{} missing",
            ADMIN_API_KEY_HEADER
        ))),
    }
}
