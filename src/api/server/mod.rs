mod convert;

#[allow(clippy::all)]
pub mod proto {
    tonic::include_proto!("services.bria.v1");
}

use futures::StreamExt;
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
    async fn import_descriptors(
        &self,
        request: Request<ImportDescriptorsRequest>,
    ) -> Result<Response<ImportDescriptorsResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let (wallet_id, xpub_ids) = self
            .app
            .import_descriptors(
                profile,
                request.wallet_name,
                request.descriptor,
                request.change_descriptor,
                request.rotate.unwrap_or(false),
            )
            .await?;
        Ok(Response::new(ImportDescriptorsResponse {
            wallet_id: wallet_id.to_string(),
            xpub_ids: xpub_ids.into_iter().map(|id| id.to_string()).collect(),
        }))
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
    async fn get_account_balance_summary(
        &self,
        request: Request<GetAccountBalanceSummaryRequest>,
    ) -> Result<Response<GetAccountBalanceSummaryResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let balance = self.app.get_account_balance_summary(profile).await?;
        Ok(Response::new(GetAccountBalanceSummaryResponse::from(
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
        let NewAddressRequest {
            wallet_name,
            external_id,
            metadata,
        } = request;

        let address = self
            .app
            .new_address(
                profile,
                wallet_name,
                external_id,
                metadata
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(BriaError::CouldNotParseIncomingMetadata)?,
            )
            .await?;
        Ok(Response::new(NewAddressResponse { address }))
    }

    #[instrument(skip_all, err)]
    async fn update_address(
        &self,
        request: Request<UpdateAddressRequest>,
    ) -> Result<Response<UpdateAddressResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let request = request.into_inner();
        let UpdateAddressRequest {
            address,
            new_external_id,
            new_metadata,
        } = request;

        self.app
            .update_address(
                profile,
                address,
                new_external_id,
                new_metadata
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(BriaError::CouldNotParseIncomingMetadata)?,
            )
            .await?;
        Ok(Response::new(UpdateAddressResponse {}))
    }

    #[instrument(skip_all, err)]
    async fn list_addresses(
        &self,
        request: Request<ListAddressesRequest>,
    ) -> Result<Response<ListAddressesResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let wallet_name = request.into_inner().wallet_name;

        let (wallet_id, addresses) = self
            .app
            .list_external_addresses(profile, wallet_name)
            .await?;
        let proto_addresses: Vec<proto::WalletAddress> = addresses
            .into_iter()
            .map(proto::WalletAddress::from)
            .collect();
        Ok(Response::new(ListAddressesResponse {
            wallet_id: wallet_id.to_string(),
            addresses: proto_addresses,
        }))
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

        let proto_keychains: Vec<proto::KeychainUtxos> = keychain_utxos
            .into_iter()
            .map(proto::KeychainUtxos::from)
            .collect();

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

        let id = self
            .app
            .queue_payout(
                profile,
                wallet_name,
                batch_group_name,
                destination.try_into()?,
                Satoshis::from(satoshis),
                None,
                metadata
                    .map(serde_json::to_value)
                    .transpose()
                    .map_err(BriaError::CouldNotParseIncomingMetadata)?,
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

    #[instrument(skip_all, err)]
    async fn list_wallets(
        &self,
        request: Request<ListWalletsRequest>,
    ) -> Result<Response<ListWalletsResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let wallets = self.app.list_wallets(profile).await?;
        let wallet_messages: Vec<proto::Wallet> =
            wallets.into_iter().map(proto::Wallet::from).collect();
        let response = ListWalletsResponse {
            wallets: wallet_messages,
        };
        Ok(Response::new(response))
    }

    #[instrument(skip_all, err)]
    async fn list_batch_groups(
        &self,
        request: Request<ListBatchGroupsRequest>,
    ) -> Result<Response<ListBatchGroupsResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let batch_groups = self.app.list_batch_groups(profile).await?;
        let batch_group_messages: Vec<proto::BatchGroup> = batch_groups
            .into_iter()
            .map(proto::BatchGroup::from)
            .collect();
        let response = ListBatchGroupsResponse {
            batch_groups: batch_group_messages,
        };
        Ok(Response::new(response))
    }

    #[instrument(skip_all, err)]
    async fn list_signing_sessions(
        &self,
        request: Request<ListSigningSessionsRequest>,
    ) -> Result<Response<ListSigningSessionsResponse>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let batch_id = request.into_inner().batch_id;
        let sessions = self
            .app
            .list_signing_sessions(
                profile,
                batch_id
                    .parse()
                    .map_err(BriaError::CouldNotParseIncomingUuid)?,
            )
            .await?;

        let session_messages: Vec<proto::SigningSession> = sessions
            .into_iter()
            .map(proto::SigningSession::from)
            .collect();
        let response = ListSigningSessionsResponse {
            sessions: session_messages,
        };
        Ok(Response::new(response))
    }

    type SubscribeAllStream = std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<BriaEvent, Status>> + Send + Sync + 'static>,
    >;

    async fn subscribe_all(
        &self,
        request: Request<SubscribeAllRequest>,
    ) -> Result<Response<Self::SubscribeAllStream>, Status> {
        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let SubscribeAllRequest {
            after_sequence,
            augment,
        } = request.into_inner();

        let outbox_listener = self
            .app
            .subscribe_all(profile, after_sequence, augment.unwrap_or(false))
            .await?;
        Ok(Response::new(Box::pin(
            outbox_listener
                .map(|event| Ok(proto::BriaEvent::from(event)))
                .fuse(),
        )))
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
