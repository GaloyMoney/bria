mod convert;

#[allow(clippy::all)]
pub mod proto {
    tonic::include_proto!("services.bria.v1");
}

use tonic::{transport::Server, Request, Response, Status};
use tracing::instrument;

use proto::{bria_service_server::BriaService, *};

use super::config::*;
use crate::{app::*, error::*, primitives::*};

pub const ACCOUNT_API_KEY_HEADER: &str = "x-bria-api-key";

pub struct Bria {
    app: App,
}

#[tonic::async_trait]
impl BriaService for Bria {
    #[instrument(skip_all, err)]
    async fn import_xpub(
        &self,
        request: Request<ImportXpubRequest>,
    ) -> Result<Response<ImportXpubResponse>, Status> {
        let key = extract_api_token(&request)?;
        let account_id = self.app.authenticate(key).await?;
        let ImportXpubRequest {
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
        Ok(Response::new(ImportXpubResponse { id: id.to_string() }))
    }

    #[instrument(skip_all, err)]
    async fn set_signer_config(
        &self,
        request: Request<SetSignerConfigRequest>,
    ) -> Result<Response<SetSignerConfigResponse>, Status> {
        let key = extract_api_token(&request)?;
        let account_id = self.app.authenticate(key).await?;
        let SetSignerConfigRequest { xpub_ref, config } = request.into_inner();
        self.app
            .set_signer_config(account_id, xpub_ref, config.try_into()?)
            .await?;
        Ok(Response::new(SetSignerConfigResponse {}))
    }

    #[instrument(skip_all, err)]
    async fn create_wallet(
        &self,
        request: Request<CreateWalletRequest>,
    ) -> Result<Response<CreateWalletResponse>, Status> {
        let key = extract_api_token(&request)?;
        let account_id = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let id = self
            .app
            .create_wallet(account_id, request.name, request.xpub_refs)
            .await?;
        Ok(Response::new(CreateWalletResponse { id: id.to_string() }))
    }

    #[instrument(skip_all, err)]
    async fn get_wallet_balance_summary(
        &self,
        request: Request<GetWalletBalanceSummaryRequest>,
    ) -> Result<Response<GetWalletBalanceSummaryResponse>, Status> {
        let key = extract_api_token(&request)?;
        let account_id = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let balance = self
            .app
            .get_wallet_balance_summary(account_id, request.wallet_name)
            .await?;

        Ok(Response::new(GetWalletBalanceSummaryResponse {
            current_settled: u64::try_from(balance.current_settled).expect("Too many sats"),
            pending_incoming: u64::try_from(balance.pending_incoming).expect("Too many sats"),
            pending_outgoing: u64::try_from(balance.pending_outgoing).expect("Too many sats"),
            encumbered_fees: u64::try_from(balance.encumbered_fees).expect("Too many sats"),
            encumbered_outgoing: u64::try_from(balance.encumbered_outgoing).expect("Too many sats"),
        }))
    }

    #[instrument(skip_all, err)]
    async fn new_address(
        &self,
        request: Request<NewAddressRequest>,
    ) -> Result<Response<NewAddressResponse>, Status> {
        let key = extract_api_token(&request)?;
        let account_id = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let address = self
            .app
            .new_address(account_id, request.wallet_name)
            .await?;
        Ok(Response::new(NewAddressResponse { address }))
    }

    #[instrument(skip_all, err)]
    async fn create_batch_group(
        &self,
        request: Request<CreateBatchGroupRequest>,
    ) -> Result<Response<CreateBatchGroupResponse>, Status> {
        let key = extract_api_token(&request)?;
        let account_id = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let id = self
            .app
            .create_batch_group(account_id, request.name)
            .await?;
        Ok(Response::new(CreateBatchGroupResponse {
            id: id.to_string(),
        }))
    }

    #[instrument(skip_all, err)]
    async fn queue_payout(
        &self,
        request: Request<QueuePayoutRequest>,
    ) -> Result<Response<QueuePayoutResponse>, Status> {
        let key = extract_api_token(&request)?;
        let account_id = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let QueuePayoutRequest {
            wallet_name,
            batch_group_name,
            destination,
            satoshis,
        } = request;

        let id = self
            .app
            .queue_payout(
                account_id,
                wallet_name,
                batch_group_name,
                destination.try_into()?,
                Satoshis::from(satoshis),
                None,
                None,
            )
            .await?;
        Ok(Response::new(QueuePayoutResponse { id: id.to_string() }))
    }
}

pub(crate) async fn start(server_config: ApiConfig, app: App) -> Result<(), BriaError> {
    let bria = Bria { app };
    println!("Starting main server on port {}", server_config.listen_port);
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
