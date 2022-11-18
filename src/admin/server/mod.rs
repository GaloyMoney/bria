mod convert;

#[allow(clippy::all)]
pub mod proto {
    tonic::include_proto!("services.admin.v1");
}

use tonic::{transport::Server, Request, Response, Status};
use tracing::instrument;

use proto::{admin_service_server::AdminService, *};

use super::{app::*, config::*, error::*};

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
        Ok(Response::new(BootstrapResponse {
            api_key: self.app.bootstrap().await?,
        }))
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
