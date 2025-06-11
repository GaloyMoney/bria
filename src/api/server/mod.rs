#![allow(clippy::blocks_in_conditions)]
mod convert;

#[allow(clippy::all)]
pub mod proto {
    tonic::include_proto!("services.bria.v1");
}

use futures::StreamExt;
use opentelemetry::propagation::{Extractor, TextMapPropagator};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tonic::{transport::Server, Request, Response, Status};
use tracing::instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;

use proto::{bria_service_server::BriaService, *};

use super::config::*;
use crate::{
    app::{error::ApplicationError, *},
    payout_queue,
    primitives::*,
    profile,
};

pub const PROFILE_API_KEY_HEADER: &str = "x-bria-api-key";

pub struct Bria {
    app: App,
}

#[tonic::async_trait]
impl BriaService for Bria {
    #[instrument(name = "bria.create_profile", skip_all, fields(error, error.level, error.message), err)]
    async fn create_profile(
        &self,
        request: Request<CreateProfileRequest>,
    ) -> Result<Response<CreateProfileResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let spending_policy = request
                .spending_policy
                .map(|policy| profile::SpendingPolicy::try_from((policy, self.app.network())))
                .transpose()?;
            let profile = self
                .app
                .create_profile(&profile, request.name, spending_policy)
                .await?;
            Ok(Response::new(CreateProfileResponse {
                id: profile.id.to_string(),
            }))
        })
        .await
    }

    #[instrument(name = "bria.update_profile", skip_all, fields(error, error.level, error.message), err)]
    async fn update_profile(
        &self,
        request: Request<UpdateProfileRequest>,
    ) -> Result<Response<UpdateProfileResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let spending_policy = request
                .spending_policy
                .map(|policy| profile::SpendingPolicy::try_from((policy, self.app.network())))
                .transpose()?;
            self.app
                .update_profile(
                    &profile,
                    request
                        .id
                        .parse()
                        .map_err(ApplicationError::CouldNotParseIncomingUuid)?,
                    spending_policy,
                )
                .await?;
            Ok(Response::new(UpdateProfileResponse {}))
        })
        .await
    }

    #[instrument(name = "bria.list_profiles", skip_all, fields(error, error.level, error.message), err)]
    async fn list_profiles(
        &self,
        request: Request<ListProfilesRequest>,
    ) -> Result<Response<ListProfilesResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let profiles = self.app.list_profiles(&profile).await?;
            let profile_messages: Vec<proto::Profile> =
                profiles.into_iter().map(proto::Profile::from).collect();
            let response = ListProfilesResponse {
                profiles: profile_messages,
            };
            Ok(Response::new(response))
        })
        .await
    }

    #[instrument(name = "bria.create_profile_api_key", skip_all, fields(error, error.level, error.message), err)]
    async fn create_profile_api_key(
        &self,
        request: Request<CreateProfileApiKeyRequest>,
    ) -> Result<Response<CreateProfileApiKeyResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let key = self
                .app
                .create_profile_api_key(&profile, request.profile_name)
                .await?;
            Ok(Response::new(CreateProfileApiKeyResponse {
                id: key.id.to_string(),
                key: key.key,
            }))
        })
        .await
    }

    #[instrument(name = "bria.import_xpub", skip_all, fields(error, error.level, error.message), err)]
    async fn import_xpub(
        &self,
        request: Request<ImportXpubRequest>,
    ) -> Result<Response<ImportXpubResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

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
                .import_xpub(&profile, name, xpub, derivation)
                .await?;
            Ok(Response::new(ImportXpubResponse { id: id.to_string() }))
        })
        .await
    }

    #[instrument(name = "bria.list_xpubs", skip_all, fields(error, error.level, error.message), err)]
    async fn list_xpubs(
        &self,
        request: Request<ListXpubsRequest>,
    ) -> Result<Response<ListXpubsResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let xpubs = self.app.list_xpubs(&profile).await?;
            let xpub_messages: Vec<proto::Xpub> =
                xpubs.into_iter().map(proto::Xpub::from).collect();
            let response = ListXpubsResponse {
                xpubs: xpub_messages,
            };
            Ok(Response::new(response))
        })
        .await
    }

    #[instrument(name = "bria.set_signer_config", skip_all, fields(error, error.level, error.message), err)]
    async fn set_signer_config(
        &self,
        request: Request<SetSignerConfigRequest>,
    ) -> Result<Response<SetSignerConfigResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let SetSignerConfigRequest { xpub_ref, config } = request.into_inner();
            self.app
                .set_signer_config(&profile, xpub_ref, config.try_into()?)
                .await?;
            Ok(Response::new(SetSignerConfigResponse {}))
        })
        .await
    }

    #[instrument(name = "bria.submit_signed_psbt", skip_all, fields(error, error.level, error.message), err)]
    async fn submit_signed_psbt(
        &self,
        request: Request<SubmitSignedPsbtRequest>,
    ) -> Result<Response<SubmitSignedPsbtResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);
            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let SubmitSignedPsbtRequest {
                batch_id,
                xpub_ref,
                signed_psbt,
            } = request;
            self.app
                .submit_signed_psbt(
                    &profile,
                    batch_id
                        .parse()
                        .map_err(ApplicationError::CouldNotParseIncomingUuid)?,
                    xpub_ref,
                    signed_psbt
                        .parse::<bitcoin::psbt::PartiallySignedTransaction>()
                        .map_err(ApplicationError::CouldNotParseIncomingPsbt)?,
                )
                .await?;
            Ok(Response::new(SubmitSignedPsbtResponse {}))
        })
        .await
    }

    #[instrument(name = "bria.create_wallet", skip_all, fields(error, error.level, error.message), err)]
    async fn create_wallet(
        &self,
        request: Request<CreateWalletRequest>,
    ) -> Result<Response<CreateWalletResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let CreateWalletRequest {
                name,
                keychain_config,
            } = request.into_inner();
            let (id, xpub_ids) = match keychain_config {
                Some(KeychainConfig {
                    config:
                        Some(keychain_config::Config::Wpkh(keychain_config::Wpkh {
                            xpub,
                            derivation_path,
                        })),
                }) => {
                    self.app
                        .create_wpkh_wallet(&profile, name, xpub, derivation_path)
                        .await?
                }
                Some(KeychainConfig {
                    config:
                        Some(keychain_config::Config::Descriptors(keychain_config::Descriptors {
                            external,
                            internal,
                        })),
                }) => {
                    self.app
                        .create_descriptors_wallet(&profile, name, external, internal)
                        .await?
                }
                Some(KeychainConfig {
                    config:
                        Some(keychain_config::Config::SortedMultisig(
                            keychain_config::SortedMultisig {
                                xpubs,
                                threshold,
                            })),
                }) => {
                    self.app.create_sorted_multisig_wallet(&profile, name, xpubs, threshold).await?
                }
                _ => {
                    return Err(Status::invalid_argument("invalid keychain config"));
                }
            };
            Ok(Response::new(CreateWalletResponse {
                id: id.to_string(),
                xpub_ids: xpub_ids.into_iter().map(|id| id.to_string()).collect(),
            }))
        })
        .await
    }

    #[instrument(name = "bria.get_wallet_balance_summary", skip_all, fields(error, error.level, error.message), err)]
    async fn get_wallet_balance_summary(
        &self,
        request: Request<GetWalletBalanceSummaryRequest>,
    ) -> Result<Response<GetWalletBalanceSummaryResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let balance = self
                .app
                .get_wallet_balance_summary(&profile, request.wallet_name)
                .await?;

            Ok(Response::new(GetWalletBalanceSummaryResponse::from(
                balance,
            )))
        })
        .await
    }

    #[instrument(name = "bria.get_account_balance_summary", skip_all, fields(error, error.level, error.message), err)]
    async fn get_account_balance_summary(
        &self,
        request: Request<GetAccountBalanceSummaryRequest>,
    ) -> Result<Response<GetAccountBalanceSummaryResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let balance = self.app.get_account_balance_summary(&profile).await?;
            Ok(Response::new(GetAccountBalanceSummaryResponse::from(
                balance,
            )))
        })
        .await
    }

    #[instrument(name = "bria.new_address", skip_all, fields(error, error.level, error.message), err)]
    async fn new_address(
        &self,
        request: Request<NewAddressRequest>,
    ) -> Result<Response<NewAddressResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let NewAddressRequest {
                wallet_name,
                external_id,
                metadata,
            } = request;

            let (_, address) = self
                .app
                .new_address(
                    &profile,
                    wallet_name,
                    external_id,
                    metadata
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(ApplicationError::CouldNotParseIncomingMetadata)?,
                )
                .await?;
            Ok(Response::new(NewAddressResponse {
                address: address.to_string(),
            }))
        })
        .await
    }

    #[instrument(name = "bria.update_address", skip_all, fields(error, error.level, error.message), err)]
    async fn update_address(
        &self,
        request: Request<UpdateAddressRequest>,
    ) -> Result<Response<UpdateAddressResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

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
                    &profile,
                    address,
                    new_external_id,
                    new_metadata
                        .map(serde_json::to_value)
                        .transpose()
                        .map_err(ApplicationError::CouldNotParseIncomingMetadata)?,
                )
                .await?;
            Ok(Response::new(UpdateAddressResponse {}))
        })
        .await
    }

    #[instrument(name = "bria.list_addresses", skip_all, fields(error, error.level, error.message), err)]
    async fn list_addresses(
        &self,
        request: Request<ListAddressesRequest>,
    ) -> Result<Response<ListAddressesResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let wallet_name = request.into_inner().wallet_name;

            let (wallet_id, addresses) = self
                .app
                .list_external_addresses(&profile, wallet_name)
                .await?;
            let proto_addresses: Vec<proto::WalletAddress> = addresses
                .into_iter()
                .map(proto::WalletAddress::from)
                .collect();
            Ok(Response::new(ListAddressesResponse {
                wallet_id: wallet_id.to_string(),
                addresses: proto_addresses,
            }))
        })
        .await
    }

    #[instrument(name = "bria.get_address", skip_all, fields(error, error.level, error.message), err)]
    async fn get_address(
        &self,
        request: Request<GetAddressRequest>,
    ) -> Result<Response<GetAddressResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);
            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let addr = match request.identifier {
                Some(get_address_request::Identifier::Address(address)) => {
                    self.app.find_address(&profile, address).await?
                }
                Some(get_address_request::Identifier::ExternalId(external_id)) => {
                    self.app
                        .find_address_by_external_id(&profile, external_id)
                        .await?
                }
                _ => {
                    return Err(Status::invalid_argument(
                        "either address or external_id must be provided",
                    ))
                }
            };
            let response = proto::GetAddressResponse::from(addr);
            Ok(Response::new(response))
        })
        .await
    }

    #[instrument(name = "bria.list_utxos", skip_all, fields(error, error.level, error.message), err)]
    async fn list_utxos(
        &self,
        request: Request<ListUtxosRequest>,
    ) -> Result<Response<ListUtxosResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let (wallet_id, keychain_utxos) =
                self.app.list_utxos(&profile, request.wallet_name).await?;

            let proto_keychains: Vec<proto::KeychainUtxos> = keychain_utxos
                .into_iter()
                .map(proto::KeychainUtxos::from)
                .collect();

            Ok(Response::new(ListUtxosResponse {
                wallet_id: wallet_id.to_string(),
                keychains: proto_keychains,
            }))
        })
        .await
    }

    #[instrument(name = "bria.create_payout_queue", skip_all, fields(error, error.level, error.message), err)]
    async fn create_payout_queue(
        &self,
        request: Request<CreatePayoutQueueRequest>,
    ) -> Result<Response<CreatePayoutQueueResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let id = self
                .app
                .create_payout_queue(
                    &profile,
                    request.name,
                    request.description,
                    request.config.map(payout_queue::PayoutQueueConfig::from),
                )
                .await?;
            Ok(Response::new(CreatePayoutQueueResponse {
                id: id.to_string(),
            }))
        })
        .await
    }

    #[instrument(name = "bria.trigger_payout_queue", skip_all, fields(error, error.level, error.message), err)]
    async fn trigger_payout_queue(
        &self,
        request: Request<TriggerPayoutQueueRequest>,
    ) -> Result<Response<TriggerPayoutQueueResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);
            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let TriggerPayoutQueueRequest { name } = request;
            self.app.trigger_payout_queue(&profile, name).await?;
            Ok(Response::new(TriggerPayoutQueueResponse {}))
        })
        .await
    }

    #[instrument(name = "bria.estimate_payout_fee", skip_all, fields(error, error.level, error.message), err)]
    async fn estimate_payout_fee(
        &self,
        request: Request<EstimatePayoutFeeRequest>,
    ) -> Result<Response<EstimatePayoutFeeResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let EstimatePayoutFeeRequest {
                wallet_name,
                payout_queue_name,
                destination,
                satoshis,
            } = request;

            let sats = match destination {
                Some(proto::estimate_payout_fee_request::Destination::OnchainAddress(address)) => {
                    self.app
                        .estimate_payout_fee_to_address(
                            &profile,
                            wallet_name,
                            payout_queue_name,
                            address,
                            Satoshis::from(satoshis),
                        )
                        .await?
                }
                Some(proto::estimate_payout_fee_request::Destination::DestinationWalletName(
                    name,
                )) => {
                    self.app
                        .estimate_payout_fee_to_wallet(
                            &profile,
                            wallet_name,
                            payout_queue_name,
                            name,
                            Satoshis::from(satoshis),
                        )
                        .await?
                }
                None => {
                    return Err(tonic::Status::new(
                        tonic::Code::InvalidArgument,
                        "missing destination",
                    ))
                }
            };
            Ok(Response::new(EstimatePayoutFeeResponse {
                satoshis: u64::from(sats),
            }))
        })
        .await
    }

    #[instrument(name = "bria.submit_payout", skip_all, fields(error, error.level, error.message), err)]
    async fn submit_payout(
        &self,
        request: Request<SubmitPayoutRequest>,
    ) -> Result<Response<SubmitPayoutResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let SubmitPayoutRequest {
                wallet_name,
                payout_queue_name,
                destination,
                satoshis,
                external_id,
                metadata,
            } = request;

            let (id, estimated_time) = match destination {
                Some(proto::submit_payout_request::Destination::OnchainAddress(address)) => {
                    self.app
                        .submit_payout_to_address(
                            &profile,
                            wallet_name,
                            payout_queue_name,
                            address,
                            Satoshis::from(satoshis),
                            external_id,
                            metadata
                                .map(serde_json::to_value)
                                .transpose()
                                .map_err(ApplicationError::CouldNotParseIncomingMetadata)?,
                        )
                        .await?
                }
                Some(proto::submit_payout_request::Destination::DestinationWalletName(name)) => {
                    self.app
                        .submit_payout_to_wallet(
                            &profile,
                            wallet_name,
                            payout_queue_name,
                            name,
                            Satoshis::from(satoshis),
                            external_id,
                            metadata
                                .map(serde_json::to_value)
                                .transpose()
                                .map_err(ApplicationError::CouldNotParseIncomingMetadata)?,
                        )
                        .await?
                }
                None => {
                    return Err(tonic::Status::new(
                        tonic::Code::InvalidArgument,
                        "missing destination",
                    ))
                }
            };
            let batch_inclusion_estimated_at = estimated_time.map(|time| time.timestamp() as u32);
            Ok(Response::new(SubmitPayoutResponse {
                id: id.to_string(),
                batch_inclusion_estimated_at,
            }))
        })
        .await
    }

    #[instrument(name = "bria.list_payouts", skip_all, fields(error, error.level, error.message), err)]
    async fn list_payouts(
        &self,
        request: Request<ListPayoutsRequest>,
    ) -> Result<Response<ListPayoutsResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let ListPayoutsRequest {
                wallet_name,
                page,
                page_size,
            } = request;
            let page = page.unwrap_or(1);
            let page_size = page_size.unwrap_or(100);
            let payouts = self
                .app
                .list_payouts(&profile, wallet_name, page, page_size)
                .await?;

            let payout_messages: Vec<proto::Payout> =
                payouts.into_iter().map(proto::Payout::from).collect();
            let response = ListPayoutsResponse {
                payouts: payout_messages,
            };
            Ok(Response::new(response))
        })
        .await
    }

    #[instrument(name = "bria.get_payout", skip_all, fields(error, error.level, error.message), err)]
    async fn get_payout(
        &self,
        request: Request<GetPayoutRequest>,
    ) -> Result<Response<GetPayoutResponse>, Status> {
        use std::str::FromStr;
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);
            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let payout = match request.identifier {
                Some(get_payout_request::Identifier::Id(id)) => {
                    if let Ok(payout_id) = PayoutId::from_str(id.as_str()) {
                        self.app.find_payout(&profile, payout_id).await?
                    } else {
                        return Err(Status::invalid_argument("could not parse the payout_id"));
                    }
                }
                Some(get_payout_request::Identifier::ExternalId(external_id)) => {
                    self.app
                        .find_payout_by_external_id(&profile, external_id)
                        .await?
                }
                _ => {
                    return Err(Status::invalid_argument(
                        "either payout_id or external_id must be provided",
                    ))
                }
            };
            let proto_payout: proto::Payout = proto::Payout::from(payout);
            Ok(Response::new(GetPayoutResponse {
                payout: Some(proto_payout),
            }))
        })
        .await
    }

    #[instrument(name = "bria.cancel_payout", skip_all, fields(error, error.level, error.message), err)]
    async fn cancel_payout(
        &self,
        request: Request<CancelPayoutRequest>,
    ) -> Result<Response<CancelPayoutResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);
            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let CancelPayoutRequest { id } = request;
            self.app
                .cancel_payout(
                    &profile,
                    id.parse()
                        .map_err(ApplicationError::CouldNotParseIncomingUuid)?,
                    false,
                )
                .await?;
            Ok(Response::new(CancelPayoutResponse {}))
        })
        .await
    }

    #[instrument(name = "bria.list_wallets", skip_all, fields(error, error.level, error.message), err)]
    async fn list_wallets(
        &self,
        request: Request<ListWalletsRequest>,
    ) -> Result<Response<ListWalletsResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let wallets = self.app.list_wallets(&profile).await?;
            let wallet_messages: Vec<proto::Wallet> =
                wallets.into_iter().map(proto::Wallet::from).collect();
            let response = ListWalletsResponse {
                wallets: wallet_messages,
            };
            Ok(Response::new(response))
        })
        .await
    }

    #[instrument(name = "bria.list_payout_queues", skip_all, fields(error, error.level, error.message), err)]
    async fn list_payout_queues(
        &self,
        request: Request<ListPayoutQueuesRequest>,
    ) -> Result<Response<ListPayoutQueuesResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let payout_queues = self.app.list_payout_queues(&profile).await?;
            let payout_queue_messages: Vec<proto::PayoutQueue> = payout_queues
                .into_iter()
                .map(proto::PayoutQueue::from)
                .collect();
            let response = ListPayoutQueuesResponse {
                payout_queues: payout_queue_messages,
            };
            Ok(Response::new(response))
        })
        .await
    }

    #[instrument(name = "bria.update_payout_queue", skip_all, fields(error, error.level, error.message), err)]
    async fn update_payout_queue(
        &self,
        request: Request<UpdatePayoutQueueRequest>,
    ) -> Result<Response<UpdatePayoutQueueResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let UpdatePayoutQueueRequest {
                id,
                new_description,
                new_config,
            } = request;

            self.app
                .update_payout_queue(
                    &profile,
                    id.parse()
                        .map_err(ApplicationError::CouldNotParseIncomingUuid)?,
                    new_description,
                    new_config.map(payout_queue::PayoutQueueConfig::from),
                )
                .await?;
            Ok(Response::new(UpdatePayoutQueueResponse {}))
        })
        .await
    }

    #[instrument(name = "bria.get_batch", skip_all, fields(error, error.level, error.message), err)]
    async fn get_batch(
        &self,
        request: Request<GetBatchRequest>,
    ) -> Result<Response<GetBatchResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);

            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let batch_id = request.into_inner().id;

            let (batch, mut payouts, sessions) = self
                .app
                .get_batch(
                    &profile,
                    batch_id
                        .parse()
                        .map_err(ApplicationError::CouldNotParseIncomingUuid)?,
                )
                .await?;
            let is_cancelled = batch.is_cancelled();
            let wallet_summaries = batch
                .wallet_summaries
                .into_iter()
                .map(|(id, summary)| {
                    proto::BatchWalletSummary::from((
                        summary,
                        payouts.remove(&id).unwrap_or_default(),
                    ))
                })
                .collect();
            Ok(Response::new(GetBatchResponse {
                id: batch.id.to_string(),
                payout_queue_id: batch.payout_queue_id.to_string(),
                tx_id: batch.bitcoin_tx_id.to_string(),
                unsigned_psbt: batch.unsigned_psbt.to_string(),
                cancelled: is_cancelled,
                wallet_summaries,
                signing_sessions: sessions
                    .map(|sessions| {
                        sessions
                            .xpub_sessions
                            .into_values()
                            .map(proto::SigningSession::from)
                            .collect()
                    })
                    .unwrap_or_default(),
            }))
        })
        .await
    }

    #[instrument(name = "bria.cancel_batch", skip_all, fields(error, error.level, error.message), err)]
    async fn cancel_batch(
        &self,
        request: Request<CancelBatchRequest>,
    ) -> Result<Response<CancelBatchResponse>, Status> {
        crate::tracing::record_error(|| async move {
            extract_tracing(&request);
            let key = extract_api_token(&request)?;
            let profile = self.app.authenticate(key).await?;
            let request = request.into_inner();
            let CancelBatchRequest { id } = request;
            self.app
                .cancel_batch(
                    &profile,
                    id.parse()
                        .map_err(ApplicationError::CouldNotParseIncomingUuid)?,
                )
                .await?;
            Ok(Response::new(CancelBatchResponse {}))
        })
        .await
    }

    type SubscribeAllStream = std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<BriaEvent, Status>> + Send + Sync + 'static>,
    >;

    #[instrument(name = "bria.subscribe_all", skip_all, fields(error, error.level, error.message), err)]
    async fn subscribe_all(
        &self,
        request: Request<SubscribeAllRequest>,
    ) -> Result<Response<Self::SubscribeAllStream>, Status> {
        extract_tracing(&request);

        let key = extract_api_token(&request)?;
        let profile = self.app.authenticate(key).await?;
        let SubscribeAllRequest {
            after_sequence,
            augment,
        } = request.into_inner();

        let outbox_listener = self
            .app
            .subscribe_all(&profile, after_sequence, augment.unwrap_or(false))
            .await?;
        Ok(Response::new(Box::pin(
            outbox_listener
                .map(|event| Ok(proto::BriaEvent::from(event)))
                .fuse(),
        )))
    }
}

pub(crate) async fn start(
    server_config: ApiConfig,
    app: App,
) -> Result<(), tonic::transport::Error> {
    use proto::bria_service_server::BriaServiceServer;

    let bria = Bria { app };
    println!("Starting main server on port {}", server_config.listen_port);
    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_serving::<BriaServiceServer<Bria>>()
        .await;
    Server::builder()
        .add_service(health_service)
        .add_service(BriaServiceServer::new(bria))
        .serve(([0, 0, 0, 0], server_config.listen_port).into())
        .await?;
    Ok(())
}

#[allow(clippy::result_large_err)]
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

pub fn extract_tracing<T>(request: &Request<T>) {
    let propagator = TraceContextPropagator::new();
    let parent_cx = propagator.extract(&RequestContextExtractor(request));
    tracing::Span::current().set_parent(parent_cx)
}

struct RequestContextExtractor<'a, T>(&'a Request<T>);

impl<T> Extractor for RequestContextExtractor<'_, T> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.metadata().get(key).and_then(|s| s.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0
            .metadata()
            .keys()
            .filter_map(|k| {
                if let tonic::metadata::KeyRef::Ascii(key) = k {
                    Some(key.as_str())
                } else {
                    None
                }
            })
            .collect()
    }
}
