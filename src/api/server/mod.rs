mod convert;

#[allow(clippy::all)]
pub mod proto {
    tonic::include_proto!("services.bria.v1");
}

use tonic::{transport::Server, Request, Response, Status};
use tracing::instrument;

use proto::{bria_service_server::BriaService, *};

use super::config::*;
use crate::{app::*, batch_group, error::*, primitives::*};

pub const PROFILE_API_KEY_HEADER: &str = "x-bria-api-key";

pub struct Bria {
    app: App,
}

#[tonic::async_trait]
impl BriaService for Bria {
    #[instrument(skip_all, err)]
    async fn create_profile(
        &self,
        request: Request<CreateProfileRequest>,
    ) -> Result<Response<CreateProfileResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let profile = self.app.create_profile(profile, request.name).await?;
        Ok(Response::new(CreateProfileResponse {
            id: profile.id.to_string(),
        }))
    }

    #[instrument(skip_all, err)]
    async fn list_profiles(
        &self,
        request: Request<ListProfilesRequest>,
    ) -> Result<Response<ListProfilesResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let profiles = self.app.list_profiles(profile).await?;
        let profile_messages: Vec<proto::Profile> =
            profiles.into_iter().map(proto::Profile::from).collect();
        let response = ListProfilesResponse {
            profiles: profile_messages,
        };
        Ok(Response::new(response))
    }

    #[instrument(skip_all, err)]
    async fn create_profile_api_key(
        &self,
        request: Request<CreateProfileApiKeyRequest>,
    ) -> Result<Response<CreateProfileApiKeyResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let key = self
            .app
            .create_profile_api_key(profile, request.profile_name)
            .await?;
        Ok(Response::new(CreateProfileApiKeyResponse {
            id: key.id.to_string(),
            key: key.key,
        }))
    }

    #[instrument(skip_all, err)]
    async fn import_xpub(
        &self,
        request: Request<ImportXpubRequest>,
    ) -> Result<Response<ImportXpubResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
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
            .import_xpub(profile, name, xpub, derivation)
            .await?;
        Ok(Response::new(ImportXpubResponse { id: id.to_string() }))
    }

    #[instrument(skip_all, err)]
    async fn set_signer_config(
        &self,
        request: Request<SetSignerConfigRequest>,
    ) -> Result<Response<SetSignerConfigResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let SetSignerConfigRequest { xpub_ref, config } = request.into_inner();
        self.app
            .set_signer_config(profile, xpub_ref, config.try_into()?)
            .await?;
        Ok(Response::new(SetSignerConfigResponse {}))
    }

    #[instrument(skip_all, err)]
    async fn create_wallet(
        &self,
        request: Request<CreateWalletRequest>,
    ) -> Result<Response<CreateWalletResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let id = self
            .app
            .create_wallet(profile, request.name, request.xpub_refs)
            .await?;
        Ok(Response::new(CreateWalletResponse { id: id.to_string() }))
    }

    #[instrument(skip_all, err)]
    async fn get_wallet_balance_summary(
        &self,
        request: Request<GetWalletBalanceSummaryRequest>,
    ) -> Result<Response<GetWalletBalanceSummaryResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let balance = self
            .app
            .get_wallet_balance_summary(profile, request.wallet_name)
            .await?;

        Ok(Response::new(GetWalletBalanceSummaryResponse::from(
            balance,
        )))
    }

    #[instrument(skip_all, err)]
    async fn new_address(
        &self,
        request: Request<NewAddressRequest>,
    ) -> Result<Response<NewAddressResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let address = self.app.new_address(profile, request.wallet_name).await?;
        Ok(Response::new(NewAddressResponse { address }))
    }

    #[instrument(skip_all, err)]
    async fn list_utxos(
        &self,
        request: Request<ListUtxosRequest>,
    ) -> Result<Response<ListUtxosResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let (wallet_id, keychain_utxos) = self.app.list_utxos(profile, request.wallet_name).await?;

        let proto_keychains: Vec<proto::KeychainUtxos> =
            keychain_utxos.into_iter().map(Into::into).collect();

        Ok(Response::new(ListUtxosResponse {
            wallet_id: wallet_id.to_string(),
            keychains: proto_keychains,
        }))
    }

    #[instrument(skip_all, err)]
    async fn create_batch_group(
        &self,
        request: Request<CreateBatchGroupRequest>,
    ) -> Result<Response<CreateBatchGroupResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let id = self
            .app
            .create_batch_group(
                profile,
                request.name,
                request.description,
                request.config.map(batch_group::BatchGroupConfig::from),
            )
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
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let QueuePayoutRequest {
            wallet_name,
            batch_group_name,
            destination,
            satoshis,
            metadata,
        } = request;

        let metadata_json = metadata
            .map(serde_json::to_value)
            .transpose()
            .map_err(BriaError::CouldNotParseIncomingMetadata)?;

        let id = self
            .app
            .queue_payout(
                profile,
                wallet_name,
                batch_group_name,
                destination.try_into()?,
                Satoshis::from(satoshis),
                None,
                metadata_json,
            )
            .await?;
        Ok(Response::new(QueuePayoutResponse { id: id.to_string() }))
    }

    #[instrument(skip_all, err)]
    async fn list_payouts(
        &self,
        request: Request<ListPayoutsRequest>,
    ) -> Result<Response<ListPayoutsResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let payouts = self
            .app
            .list_payouts(profile, request.into_inner().wallet_name)
            .await?;

        let payout_messages: Vec<proto::Payout> =
            payouts.into_iter().map(proto::Payout::from).collect();
        let response = ListPayoutsResponse {
            payouts: payout_messages,
        };
        Ok(Response::new(response))
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
    match request.metadata().get(PROFILE_API_KEY_HEADER) {
        Some(value) => value
            .to_str()
            .map_err(|_| Status::unauthenticated("Bad token")),
        None => Err(Status::unauthenticated(format!(
            "{PROFILE_API_KEY_HEADER} missing"
        ))),
    }
}
