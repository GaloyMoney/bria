use std::time::Duration;

use super::proto;
use crate::{
    account::balance::AccountBalanceSummary,
    address::*,
    app::error::*,
    batch::*,
    batch_inclusion::PayoutWithInclusionEstimate,
    outbox::*,
    payout::*,
    payout_queue::*,
    primitives::{bitcoin::*, *},
    profile::*,
    signing_session::*,
    tracing::ToTraceLevel,
    utxo::*,
    wallet::balance::WalletBalanceSummary,
    wallet::*,
    xpub::*,
};

impl From<Profile> for proto::Profile {
    fn from(p: Profile) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name,
            spending_policy: p.spending_policy.map(proto::SpendingPolicy::from),
        }
    }
}

impl From<SpendingPolicy> for proto::SpendingPolicy {
    fn from(sp: SpendingPolicy) -> Self {
        Self {
            allowed_payout_addresses: sp
                .allowed_payout_addresses
                .into_iter()
                .map(|addr| addr.to_string())
                .collect(),
            max_payout_sats: sp.max_payout.map(u64::from),
        }
    }
}

impl TryFrom<(proto::SpendingPolicy, bitcoin::Network)> for SpendingPolicy {
    type Error = tonic::Status;

    fn try_from(
        (sp, network): (proto::SpendingPolicy, bitcoin::Network),
    ) -> Result<Self, Self::Error> {
        let mut allowed_payout_addresses = Vec::new();
        for dest in sp.allowed_payout_addresses {
            let addr = Address::try_from((dest, network))
                .map_err(|err| tonic::Status::invalid_argument(err.to_string()))?;
            allowed_payout_addresses.push(addr);
        }
        Ok(Self {
            allowed_payout_addresses,
            max_payout: sp.max_payout_sats.map(Satoshis::from),
        })
    }
}

impl TryFrom<Option<proto::set_signer_config_request::Config>> for SignerConfig {
    type Error = tonic::Status;

    fn try_from(
        config: Option<proto::set_signer_config_request::Config>,
    ) -> Result<Self, Self::Error> {
        match config {
            Some(proto::set_signer_config_request::Config::Lnd(config)) => {
                Ok(SignerConfig::Lnd(LndSignerConfig {
                    endpoint: config.endpoint,
                    cert_base64: config.cert_base64,
                    macaroon_base64: config.macaroon_base64,
                }))
            }
            Some(proto::set_signer_config_request::Config::Bitcoind(config)) => {
                Ok(SignerConfig::Bitcoind(BitcoindSignerConfig {
                    endpoint: config.endpoint,
                    rpc_user: config.rpc_user,
                    rpc_password: config.rpc_password,
                }))
            }
            None => Err(tonic::Status::new(
                tonic::Code::InvalidArgument,
                "missing signer config",
            )),
        }
    }
}

impl From<WalletAddress> for proto::WalletAddress {
    fn from(addr: WalletAddress) -> Self {
        Self {
            address: addr.address.to_string(),
            metadata: addr.metadata().map(|json| {
                serde_json::from_value(json.clone()).expect("Could not transfer json -> struct")
            }),
            external_id: addr.external_id,
        }
    }
}

impl From<WalletAddress> for proto::GetAddressResponse {
    fn from(addr: WalletAddress) -> Self {
        let wallet_id = addr.wallet_id.to_string();
        let change_address = !addr.is_external();
        let (address, metadata, external_id) = if change_address {
            (None, None, None)
        } else {
            (
                Some(addr.address.to_string()),
                addr.metadata().map(|json| {
                    serde_json::from_value(json.clone()).expect("Could not transfer json -> struct")
                }),
                Some(addr.external_id),
            )
        };
        Self {
            address,
            wallet_id,
            change_address,
            external_id,
            metadata,
        }
    }
}

impl From<AccountXPub> for proto::Xpub {
    fn from(xpub: AccountXPub) -> Self {
        Self {
            name: xpub.key_name.to_string(),
            id: xpub.fingerprint().to_string(),
            xpub: xpub.original.clone(),
            derivation_path: xpub
                .derivation_path()
                .map(|derivation_path| derivation_path.to_string()),
            has_signer_config: xpub.has_signer_config(),
        }
    }
}

impl From<WalletUtxo> for proto::Utxo {
    fn from(utxo: WalletUtxo) -> Self {
        Self {
            outpoint: format!("{}:{}", utxo.outpoint.txid, utxo.outpoint.vout),
            address_idx: utxo.address_idx,
            value: u64::from(utxo.value),
            address: utxo.address.map(|a| a.to_string()),
            change_output: utxo.kind == KeychainKind::Internal,
            block_height: utxo.block_height,
        }
    }
}

impl From<KeychainUtxos> for proto::KeychainUtxos {
    fn from(keychain_utxo: KeychainUtxos) -> Self {
        Self {
            keychain_id: keychain_utxo.keychain_id.to_string(),
            utxos: keychain_utxo.utxos.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<PayoutWithInclusionEstimate> for proto::Payout {
    fn from(
        PayoutWithInclusionEstimate {
            payout,
            estimated_batch_inclusion,
        }: PayoutWithInclusionEstimate,
    ) -> Self {
        let cancelled = payout.is_cancelled();
        let destination = match payout.destination {
            PayoutDestination::OnchainAddress { value } => {
                proto::payout::Destination::OnchainAddress(value.to_string())
            }
            PayoutDestination::Wallet { id, address } => {
                proto::payout::Destination::Wallet(proto::BriaWalletDestination {
                    wallet_id: id.to_string(),
                    address: address.to_string(),
                })
            }
        };

        let (tx_id, vout) = payout.outpoint.map_or((None, None), |outpoint| {
            (Some(outpoint.txid.to_string()), Some(outpoint.vout))
        });

        let batch_inclusion_estimated_at =
            estimated_batch_inclusion.map(|time| time.timestamp() as u32);
        proto::Payout {
            id: payout.id.to_string(),
            wallet_id: payout.wallet_id.to_string(),
            payout_queue_id: payout.payout_queue_id.to_string(),
            batch_id: payout.batch_id.map(|id| id.to_string()),
            satoshis: u64::from(payout.satoshis),
            destination: Some(destination),
            cancelled,
            external_id: payout.external_id,
            metadata: payout.metadata.map(|json| {
                serde_json::from_value(json).expect("Could not transfer json -> struct")
            }),
            batch_inclusion_estimated_at,
            tx_id,
            vout,
        }
    }
}

impl From<Wallet> for proto::Wallet {
    fn from(wallet: Wallet) -> Self {
        let id = wallet.id.to_string();
        let name = wallet.name;
        let config: proto::WalletConfig = proto::WalletConfig::from(wallet.config);
        proto::Wallet {
            id,
            name,
            config: Some(config),
        }
    }
}

impl From<WalletConfig> for proto::WalletConfig {
    fn from(config: WalletConfig) -> Self {
        Self {
            settle_income_after_n_confs: config.settle_income_after_n_confs,
            settle_change_after_n_confs: config.settle_change_after_n_confs,
        }
    }
}

impl From<PayoutQueue> for proto::PayoutQueue {
    fn from(payout_queue: PayoutQueue) -> Self {
        let id = payout_queue.id.to_string();
        let description = payout_queue.description();
        let name = payout_queue.name;
        let trigger = match payout_queue.config.trigger {
            PayoutQueueTrigger::Interval { seconds } => {
                proto::payout_queue_config::Trigger::IntervalSecs(seconds.as_secs() as u32)
            }
            PayoutQueueTrigger::Manual => proto::payout_queue_config::Trigger::Manual(true),
        };
        let tx_priority: proto::TxPriority = payout_queue.config.tx_priority.into();
        let config = Some(proto::PayoutQueueConfig {
            trigger: Some(trigger),
            tx_priority: tx_priority as i32,
            consolidate_deprecated_keychains: payout_queue.config.consolidate_deprecated_keychains,
            cpfp_payouts_after_mins: payout_queue.config.cpfp_payouts_after_mins,
            cpfp_payouts_after_blocks: payout_queue.config.cpfp_payouts_after_blocks,
            force_min_change_sats: payout_queue.config.force_min_change_sats.map(u64::from),
        });
        proto::PayoutQueue {
            id,
            name,
            description,
            config,
        }
    }
}

impl From<TxPriority> for proto::TxPriority {
    fn from(priority: TxPriority) -> Self {
        match priority {
            TxPriority::NextBlock => proto::TxPriority::NextBlock,
            TxPriority::HalfHour => proto::TxPriority::HalfHour,
            TxPriority::OneHour => proto::TxPriority::OneHour,
        }
    }
}

impl From<SigningSession> for proto::SigningSession {
    fn from(session: SigningSession) -> Self {
        proto::SigningSession {
            id: session.id.to_string(),
            batch_id: session.batch_id.to_string(),
            xpub_id: session.xpub_fingerprint.to_string(),
            failure_reason: session.failure_reason().map(|r| r.to_string()),
            state: format!("{:?}", session.state()),
        }
    }
}

impl From<proto::PayoutQueueConfig> for PayoutQueueConfig {
    fn from(proto_config: proto::PayoutQueueConfig) -> Self {
        let tx_priority =
            proto::TxPriority::try_from(proto_config.tx_priority).map(TxPriority::from);

        let trigger = match proto_config.trigger {
            Some(proto::payout_queue_config::Trigger::IntervalSecs(interval)) => {
                Some(PayoutQueueTrigger::Interval {
                    seconds: Duration::from_secs(interval as u64),
                })
            }
            Some(proto::payout_queue_config::Trigger::Manual(true)) => {
                Some(PayoutQueueTrigger::Manual)
            }
            _ => None,
        };

        let mut ret = Self {
            consolidate_deprecated_keychains: proto_config.consolidate_deprecated_keychains,
            cpfp_payouts_after_mins: proto_config.cpfp_payouts_after_mins,
            cpfp_payouts_after_blocks: proto_config.cpfp_payouts_after_blocks,
            force_min_change_sats: proto_config.force_min_change_sats.map(Satoshis::from),
            ..Self::default()
        };

        if let Some(trigger) = trigger {
            ret.trigger = trigger;
        }
        if let Ok(tx_priority) = tx_priority {
            ret.tx_priority = tx_priority;
        }
        ret
    }
}

impl From<proto::TxPriority> for TxPriority {
    fn from(proto_tx_priority: proto::TxPriority) -> Self {
        match proto_tx_priority {
            proto::TxPriority::NextBlock => TxPriority::NextBlock,
            proto::TxPriority::HalfHour => TxPriority::HalfHour,
            proto::TxPriority::OneHour => TxPriority::OneHour,
        }
    }
}

impl From<(WalletSummary, Vec<Payout>)> for proto::BatchWalletSummary {
    fn from((summary, payouts): (WalletSummary, Vec<Payout>)) -> Self {
        Self {
            wallet_id: summary.wallet_id.to_string(),
            total_spent_sats: u64::from(summary.total_spent_sats),
            fee_sats: u64::from(summary.total_fee_sats),
            payouts: payouts
                .into_iter()
                .map(|payout| {
                    let destination = match payout.destination {
                        PayoutDestination::OnchainAddress { value } => {
                            proto::payout_summary::Destination::OnchainAddress(value.to_string())
                        }
                        PayoutDestination::Wallet { id, address } => {
                            proto::payout_summary::Destination::Wallet(
                                proto::BriaWalletDestination {
                                    wallet_id: id.to_string(),
                                    address: address.to_string(),
                                },
                            )
                        }
                    };
                    proto::PayoutSummary {
                        id: payout.id.to_string(),
                        satoshis: u64::from(payout.satoshis),
                        destination: Some(destination),
                    }
                })
                .collect(),
        }
    }
}

impl From<WalletBalanceSummary> for proto::GetWalletBalanceSummaryResponse {
    fn from(balance: WalletBalanceSummary) -> Self {
        let has_negative_balance = [
            balance.utxo_settled.is_negative(),
            balance.utxo_pending_incoming.is_negative(),
            balance.utxo_pending_outgoing.is_negative(),
            balance.utxo_encumbered_incoming.is_negative(),
            balance.fees_pending.is_negative(),
            balance.fees_encumbered.is_negative(),
            balance.effective_settled.is_negative(),
            balance.effective_pending_income.is_negative(),
            balance.effective_pending_outgoing.is_negative(),
            balance.effective_encumbered_outgoing.is_negative(),
        ]
        .iter()
        .any(|&x| x);

        if has_negative_balance {
            tracing::Span::current().record("error", true);
            tracing::Span::current().record(
                "error.message",
                "Negative balance values detected in wallet summary",
            );
            tracing::Span::current().record(
                "error.level",
                tracing::field::display(tracing::Level::ERROR),
            );
        }

        Self {
            utxo_encumbered_incoming: u64::from(
                balance.utxo_encumbered_incoming.max(Satoshis::ZERO),
            ),
            utxo_pending_incoming: u64::from(balance.utxo_pending_incoming.max(Satoshis::ZERO)),
            utxo_settled: u64::from(balance.utxo_settled.max(Satoshis::ZERO)),
            utxo_pending_outgoing: u64::from(balance.utxo_pending_outgoing.max(Satoshis::ZERO)),
            fees_pending: u64::from(balance.fees_pending.max(Satoshis::ZERO)),
            fees_encumbered: u64::from(balance.fees_encumbered.max(Satoshis::ZERO)),
            effective_pending_income: u64::from(
                balance.effective_pending_income.max(Satoshis::ZERO),
            ),
            effective_settled: u64::from(balance.effective_settled.max(Satoshis::ZERO)),
            effective_pending_outgoing: u64::from(
                balance.effective_pending_outgoing.max(Satoshis::ZERO),
            ),
            effective_encumbered_outgoing: u64::from(
                balance.effective_encumbered_outgoing.max(Satoshis::ZERO),
            ),
        }
    }
}

impl From<AccountBalanceSummary> for proto::GetAccountBalanceSummaryResponse {
    fn from(balance: AccountBalanceSummary) -> Self {
        Self {
            utxo_encumbered_incoming: u64::from(balance.utxo_encumbered_incoming),
            utxo_pending_incoming: u64::from(balance.utxo_pending_incoming),
            utxo_settled: u64::from(balance.utxo_settled),
            utxo_pending_outgoing: u64::from(balance.utxo_pending_outgoing),
            fees_pending: u64::from(balance.fees_pending),
            fees_encumbered: u64::from(balance.fees_encumbered),
            effective_pending_income: u64::from(balance.effective_pending_income),
            effective_settled: u64::from(balance.effective_settled),
            effective_pending_outgoing: u64::from(balance.effective_pending_outgoing),
            effective_encumbered_outgoing: u64::from(balance.effective_encumbered_outgoing),
        }
    }
}

impl From<OutboxEvent<Augmentation>> for proto::BriaEvent {
    fn from(event: OutboxEvent<Augmentation>) -> Self {
        let payload = match event.payload {
            OutboxEventPayload::UtxoDetected {
                tx_id,
                vout,
                satoshis,
                address,
                wallet_id,
                ..
            } => proto::bria_event::Payload::UtxoDetected(proto::UtxoDetected {
                wallet_id: wallet_id.to_string(),
                tx_id: tx_id.to_string(),
                vout,
                satoshis: u64::from(satoshis),
                address: address.to_string(),
            }),
            OutboxEventPayload::UtxoSettled {
                tx_id,
                vout,
                satoshis,
                address,
                wallet_id,
                confirmation_time,
                ..
            } => proto::bria_event::Payload::UtxoSettled(proto::UtxoSettled {
                wallet_id: wallet_id.to_string(),
                tx_id: tx_id.to_string(),
                vout,
                satoshis: u64::from(satoshis),
                address: address.to_string(),
                block_height: confirmation_time.height,
                block_time: confirmation_time.timestamp,
            }),
            OutboxEventPayload::UtxoDropped {
                tx_id,
                vout,
                satoshis,
                address,
                wallet_id,
                ..
            } => proto::bria_event::Payload::UtxoDropped(proto::UtxoDropped {
                wallet_id: wallet_id.to_string(),
                tx_id: tx_id.to_string(),
                vout,
                satoshis: u64::from(satoshis),
                address: address.to_string(),
            }),
            OutboxEventPayload::PayoutSubmitted {
                id,
                wallet_id,
                payout_queue_id,
                satoshis,
                destination,
                ..
            } => proto::bria_event::Payload::PayoutSubmitted(proto::PayoutSubmitted {
                id: id.to_string(),
                wallet_id: wallet_id.to_string(),
                payout_queue_id: payout_queue_id.to_string(),
                satoshis: u64::from(satoshis),
                destination: Some(match destination {
                    PayoutDestination::OnchainAddress { value: destination } => {
                        proto::payout_submitted::Destination::OnchainAddress(
                            destination.to_string(),
                        )
                    }
                    PayoutDestination::Wallet { id, address } => {
                        proto::payout_submitted::Destination::Wallet(proto::BriaWalletDestination {
                            wallet_id: id.to_string(),
                            address: address.to_string(),
                        })
                    }
                }),
            }),
            OutboxEventPayload::PayoutCancelled {
                id,
                wallet_id,
                payout_queue_id,
                satoshis,
                destination,
                ..
            } => proto::bria_event::Payload::PayoutCancelled(proto::PayoutCancelled {
                id: id.to_string(),
                wallet_id: wallet_id.to_string(),
                payout_queue_id: payout_queue_id.to_string(),
                satoshis: u64::from(satoshis),
                destination: Some(match destination {
                    PayoutDestination::OnchainAddress { value: destination } => {
                        proto::payout_cancelled::Destination::OnchainAddress(
                            destination.to_string(),
                        )
                    }
                    PayoutDestination::Wallet { id, address } => {
                        proto::payout_cancelled::Destination::Wallet(proto::BriaWalletDestination {
                            wallet_id: id.to_string(),
                            address: address.to_string(),
                        })
                    }
                }),
            }),
            OutboxEventPayload::PayoutCommitted {
                id,
                tx_id,
                vout,
                wallet_id,
                payout_queue_id,
                satoshis,
                destination,
                proportional_fee,
                ..
            } => proto::bria_event::Payload::PayoutCommitted(proto::PayoutCommitted {
                id: id.to_string(),
                tx_id: tx_id.to_string(),
                vout,
                wallet_id: wallet_id.to_string(),
                payout_queue_id: payout_queue_id.to_string(),
                satoshis: u64::from(satoshis),
                destination: Some(match destination {
                    PayoutDestination::OnchainAddress { value: destination } => {
                        proto::payout_committed::Destination::OnchainAddress(
                            destination.to_string(),
                        )
                    }
                    PayoutDestination::Wallet { id, address } => {
                        proto::payout_committed::Destination::Wallet(proto::BriaWalletDestination {
                            wallet_id: id.to_string(),
                            address: address.to_string(),
                        })
                    }
                }),
                proportional_fee_sats: u64::from(proportional_fee),
            }),
            OutboxEventPayload::PayoutBroadcast {
                id,
                tx_id,
                vout,
                wallet_id,
                payout_queue_id,
                satoshis,
                destination,
                proportional_fee,
                ..
            } => proto::bria_event::Payload::PayoutBroadcast(proto::PayoutBroadcast {
                id: id.to_string(),
                tx_id: tx_id.to_string(),
                vout,
                wallet_id: wallet_id.to_string(),
                payout_queue_id: payout_queue_id.to_string(),
                satoshis: u64::from(satoshis),
                destination: Some(match destination {
                    PayoutDestination::OnchainAddress { value: destination } => {
                        proto::payout_broadcast::Destination::OnchainAddress(
                            destination.to_string(),
                        )
                    }
                    PayoutDestination::Wallet { id, address } => {
                        proto::payout_broadcast::Destination::Wallet(proto::BriaWalletDestination {
                            wallet_id: id.to_string(),
                            address: address.to_string(),
                        })
                    }
                }),
                proportional_fee_sats: u64::from(proportional_fee),
            }),
            OutboxEventPayload::PayoutSettled {
                id,
                tx_id,
                vout,
                wallet_id,
                payout_queue_id,
                satoshis,
                destination,
                proportional_fee,
                ..
            } => proto::bria_event::Payload::PayoutSettled(proto::PayoutSettled {
                id: id.to_string(),
                tx_id: tx_id.to_string(),
                vout,
                wallet_id: wallet_id.to_string(),
                payout_queue_id: payout_queue_id.to_string(),
                satoshis: u64::from(satoshis),
                destination: Some(match destination {
                    PayoutDestination::OnchainAddress { value: destination } => {
                        proto::payout_settled::Destination::OnchainAddress(destination.to_string())
                    }
                    PayoutDestination::Wallet { id, address } => {
                        proto::payout_settled::Destination::Wallet(proto::BriaWalletDestination {
                            wallet_id: id.to_string(),
                            address: address.to_string(),
                        })
                    }
                }),
                proportional_fee_sats: u64::from(proportional_fee),
            }),
        };

        let augmentation = event.augmentation.map(|a| proto::EventAugmentation {
            address_info: a.address.map(proto::WalletAddress::from),
            payout_info: a.payout.map(proto::Payout::from),
        });
        proto::BriaEvent {
            sequence: u64::from(event.sequence),
            payload: Some(payload),
            recorded_at: event.recorded_at.timestamp() as u32,
            augmentation,
        }
    }
}

impl From<AddressAugmentation> for proto::WalletAddress {
    fn from(addr: AddressAugmentation) -> Self {
        Self {
            address: addr.address.to_string(),
            metadata: addr.metadata.map(|json| {
                serde_json::from_value(json).expect("Could not transfer json -> struct")
            }),
            external_id: addr.external_id,
        }
    }
}

impl From<ApplicationError> for tonic::Status {
    fn from(err: ApplicationError) -> Self {
        use crate::{address::error::*, payout::error::*, profile::error::*, wallet::error::*};

        match err {
            ApplicationError::ProfileError(ProfileError::ProfileKeyNotFound) => {
                tonic::Status::unauthenticated(err.to_string())
            }
            ApplicationError::WalletError(err) if err.was_not_found() => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::AddressError(err) if err.was_not_found() => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::AddressError(AddressError::ExternalIdAlreadyExists) => {
                tonic::Status::already_exists(err.to_string())
            }
            ApplicationError::PayoutQueueError(err) if err.was_not_found() => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::ProfileError(err) if err.was_not_found() => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::PayoutError(err) if err.was_not_found() => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::PayoutError(PayoutError::ExternalIdAlreadyExists) => {
                tonic::Status::already_exists(err.to_string())
            }
            ApplicationError::CouldNotParseIncomingMetadata(_) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            ApplicationError::CouldNotParseIncomingUuid(_) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            ApplicationError::DestinationBlocked(_) => {
                tonic::Status::permission_denied(err.to_string())
            }
            ApplicationError::DestinationNotAllowed(_) => {
                tonic::Status::permission_denied(err.to_string())
            }
            ApplicationError::PayoutExceedsMaximum(_) => {
                tonic::Status::permission_denied(err.to_string())
            }
            ApplicationError::SigningSessionNotFoundForBatchId(_) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::SigningSessionNotFoundForXPubFingerprint(_) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::WalletError(WalletError::PsbtDoesNotHaveValidSignatures) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            ApplicationError::WalletError(WalletError::UnsignedTxnMismatch) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            ApplicationError::CouldNotParseIncomingPsbt(_) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            ApplicationError::PayoutError(PayoutError::PayoutAlreadyCommitted) => {
                tonic::Status::failed_precondition(err.to_string())
            }
            ApplicationError::PayoutError(PayoutError::PayoutAlreadyCancelled) => {
                tonic::Status::failed_precondition(err.to_string())
            }
            ApplicationError::CouldNotParseAddress(_) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            _ => tonic::Status::internal(err.to_string()),
        }
    }
}

impl ToTraceLevel for tonic::Status {
    fn to_trace_level(&self) -> tracing::Level {
        match self.code() {
            tonic::Code::NotFound => tracing::Level::WARN,
            tonic::Code::AlreadyExists => tracing::Level::WARN,
            tonic::Code::PermissionDenied => tracing::Level::WARN,
            tonic::Code::InvalidArgument => tracing::Level::WARN,
            tonic::Code::FailedPrecondition => tracing::Level::WARN,
            _ => tracing::Level::ERROR,
        }
    }
}
