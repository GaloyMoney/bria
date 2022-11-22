mod convert;

#[allow(clippy::all)]
pub mod proto {
    tonic::include_proto!("services.bria.v1");
}

use tonic::{transport::Server, Request, Response, Status};
use tracing::instrument;

use proto::{bria_service_server::BriaService, *};

use super::config::*;
use crate::{app::*, error::*};

pub const ACCOUNT_API_KEY_HEADER: &str = "x-bria-api-key";

pub struct Bria {
    app: App,
}

#[tonic::async_trait]
impl BriaService for Bria {
    #[instrument(skip_all, err)]
    async fn x_pub_import(
        &self,
        request: Request<XPubImportRequest>,
    ) -> Result<Response<XPubImportResponse>, Status> {
        let key = extract_api_token(&request)?;
        let account_id = self.app.authenticate(key).await?;
        let XPubImportRequest {
            name,
            xpub,
            derivation,
        } = request.into_inner();
        let derivation = if derivation.is_empty() {
            None
        } else {
            Some(derivation)
        };
        let id = self
            .app
            .import_xpub(account_id, name, xpub, derivation)
            .await?;
        Ok(Response::new(XPubImportResponse { id: id.to_string() }))
    }

    #[instrument(skip_all, err)]
    async fn wallet_create(
        &self,
        request: Request<WalletCreateRequest>,
    ) -> Result<Response<WalletCreateResponse>, Status> {
        let key = extract_api_token(&request)?;
        let account_id = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let id = self
            .app
            .create_wallet(account_id, request.name, request.xpub_refs)
            .await?;
        Ok(Response::new(WalletCreateResponse { id: id.to_string() }))
    }
}

pub(crate) async fn start(server_config: ApiConfig, app: App) -> Result<(), BriaError> {
    let bria = Bria { app };
    Server::builder()
        .add_service(proto::bria_service_server::BriaServiceServer::new(bria))
        .serve(([0, 0, 0, 0], server_config.listen_port).into())
        .await?;
    Ok(())
}

pub fn extract_api_token<T>(request: &Request<T>) -> Result<&str, Status> {
    match request.metadata().get(ACCOUNT_API_KEY_HEADER) {
        Some(value) => value
            .to_str()
            .map_err(|_| Status::unauthenticated("Bad token")),
        None => Err(Status::unauthenticated(format!(
            "{} missing",
            ACCOUNT_API_KEY_HEADER
        ))),
    }
}
