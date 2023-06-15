use std::time::Duration;

use super::proto;
use crate::{
    account::balance::AccountBalanceSummary,
    address::*,
    app::error::*,
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
        }
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

impl TryFrom<Option<proto::estimate_payout_fee_request::Destination>> for PayoutDestination {
    type Error = tonic::Status;

    fn try_from(
        destination: Option<proto::estimate_payout_fee_request::Destination>,
    ) -> Result<Self, Self::Error> {
        match destination {
            Some(proto::estimate_payout_fee_request::Destination::OnchainAddress(destination)) => {
                Ok(PayoutDestination::OnchainAddress {
                    value: destination.parse().map_err(|_| {
                        tonic::Status::new(
                            tonic::Code::InvalidArgument,
                            "on chain address couldn't be parsed",
                        )
                    })?,
                })
            }
            None => Err(tonic::Status::new(
                tonic::Code::InvalidArgument,
                "missing destination",
            )),
        }
    }
}

impl TryFrom<Option<proto::submit_payout_request::Destination>> for PayoutDestination {
    type Error = tonic::Status;

    fn try_from(
        destination: Option<proto::submit_payout_request::Destination>,
    ) -> Result<Self, Self::Error> {
        match destination {
            Some(proto::submit_payout_request::Destination::OnchainAddress(destination)) => {
                Ok(PayoutDestination::OnchainAddress {
                    value: destination.parse().map_err(|_| {
                        tonic::Status::new(
                            tonic::Code::InvalidArgument,
                            "on chain address couldn't be parsed",
                        )
                    })?,
                })
            }
            None => Err(tonic::Status::new(
                tonic::Code::InvalidArgument,
                "missing destination",
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

impl From<AccountXPub> for proto::Xpub {
    fn from(xpub: AccountXPub) -> Self {
        Self {
            name: xpub.key_name.to_string(),
            id: xpub.id().to_string(),
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

impl From<Payout> for proto::Payout {
    fn from(payout: Payout) -> Self {
        let destination = match payout.destination {
            PayoutDestination::OnchainAddress { value } => {
                proto::payout::Destination::OnchainAddress(value.to_string())
            }
        };

        proto::Payout {
            id: payout.id.to_string(),
            wallet_id: payout.wallet_id.to_string(),
            payout_queue_id: payout.payout_queue_id.to_string(),
            batch_id: payout.batch_id.map(|id| id.to_string()),
            satoshis: u64::from(payout.satoshis),
            destination: Some(destination),
            external_id: payout.external_id,
            metadata: payout.metadata.map(|json| {
                serde_json::from_value(json).expect("Could not transfer json -> struct")
            }),
        }
    }
}

impl From<Wallet> for proto::Wallet {
    fn from(wallet: Wallet) -> Self {
        let id = wallet.id.to_string();
        let name = wallet.name as String;
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
        let consolidate_deprecated_keychains = payout_queue.config.consolidate_deprecated_keychains;
        let trigger = match payout_queue.config.trigger {
            PayoutQueueTrigger::Interval { seconds } => {
                proto::payout_queue_config::Trigger::IntervalSecs(seconds.as_secs() as u32)
            }
        };
        let tx_priority: proto::TxPriority = payout_queue.config.tx_priority.into();
        let config = Some(proto::PayoutQueueConfig {
            trigger: Some(trigger),
            tx_priority: tx_priority as i32,
            consolidate_deprecated_keychains,
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
            xpub_id: session.xpub_id.to_string(),
            failure_reason: session.failure_reason().map(|r| r.to_string()),
            state: format!("{:?}", session.state()),
        }
    }
}

impl From<proto::PayoutQueueConfig> for PayoutQueueConfig {
    fn from(proto_config: proto::PayoutQueueConfig) -> Self {
        let tx_priority =
            proto::TxPriority::from_i32(proto_config.tx_priority).map(TxPriority::from);
        let consolidate_deprecated_keychains = proto_config.consolidate_deprecated_keychains;

        let trigger = if let Some(proto::payout_queue_config::Trigger::IntervalSecs(interval)) =
            proto_config.trigger
        {
            Some(PayoutQueueTrigger::Interval {
                seconds: Duration::from_secs(interval as u64),
            })
        } else {
            None
        };

        let mut ret = Self {
            consolidate_deprecated_keychains,
            ..Self::default()
        };

        if let Some(trigger) = trigger {
            ret.trigger = trigger;
        }
        if let Some(tx_priority) = tx_priority {
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

impl From<WalletBalanceSummary> for proto::GetWalletBalanceSummaryResponse {
    fn from(balance: WalletBalanceSummary) -> Self {
        Self {
            utxo_encumbered_incoming: u64::try_from(balance.utxo_encumbered_incoming)
                .expect("Satoshis -> u64 failed"),
            utxo_pending_incoming: u64::try_from(balance.utxo_pending_incoming)
                .expect("Satoshis -> u64 failed"),
            utxo_settled: u64::try_from(balance.utxo_settled).expect("Satoshis -> u64 failed"),
            utxo_pending_outgoing: u64::try_from(balance.utxo_pending_outgoing)
                .expect("Satoshis -> u64 failed"),
            fees_pending: u64::try_from(balance.fees_pending).expect("Satoshis -> u64 failed"),
            fees_encumbered: u64::try_from(balance.fees_encumbered)
                .expect("Satoshis -> u64 failed"),
            effective_pending_income: u64::try_from(balance.effective_pending_income)
                .expect("Satoshis -> u64 failed"),
            effective_settled: u64::try_from(balance.effective_settled)
                .expect("Satoshis -> u64 failed"),
            effective_pending_outgoing: u64::try_from(balance.effective_pending_outgoing)
                .expect("Satoshis -> u64 failed"),
            effective_encumbered_outgoing: u64::try_from(balance.effective_encumbered_outgoing)
                .expect("Satoshis -> u64 failed"),
        }
    }
}

impl From<AccountBalanceSummary> for proto::GetAccountBalanceSummaryResponse {
    fn from(balance: AccountBalanceSummary) -> Self {
        Self {
            utxo_encumbered_incoming: u64::try_from(balance.utxo_encumbered_incoming)
                .expect("Satoshis -> u64 failed"),
            utxo_pending_incoming: u64::try_from(balance.utxo_pending_incoming)
                .expect("Satoshis -> u64 failed"),
            utxo_settled: u64::try_from(balance.utxo_settled).expect("Satoshis -> u64 failed"),
            utxo_pending_outgoing: u64::try_from(balance.utxo_pending_outgoing)
                .expect("Satoshis -> u64 failed"),
            fees_pending: u64::try_from(balance.fees_pending).expect("Satoshis -> u64 failed"),
            fees_encumbered: u64::try_from(balance.fees_encumbered)
                .expect("Satoshis -> u64 failed"),
            effective_pending_income: u64::try_from(balance.effective_pending_income)
                .expect("Satoshis -> u64 failed"),
            effective_settled: u64::try_from(balance.effective_settled)
                .expect("Satoshis -> u64 failed"),
            effective_pending_outgoing: u64::try_from(balance.effective_pending_outgoing)
                .expect("Satoshis -> u64 failed"),
            effective_encumbered_outgoing: u64::try_from(balance.effective_encumbered_outgoing)
                .expect("Satoshis -> u64 failed"),
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
            OutboxEventPayload::PayoutSubmitted {
                id,
                wallet_id,
                payout_queue_id,
                satoshis,
                destination: PayoutDestination::OnchainAddress { value: destination },
                ..
            } => proto::bria_event::Payload::PayoutSubmitted(proto::PayoutSubmitted {
                id: id.to_string(),
                wallet_id: wallet_id.to_string(),
                payout_queue_id: payout_queue_id.to_string(),
                satoshis: u64::from(satoshis),
                destination: Some(proto::payout_submitted::Destination::OnchainAddress(
                    destination.to_string(),
                )),
            }),
            OutboxEventPayload::PayoutCommitted {
                id,
                tx_id,
                vout,
                wallet_id,
                payout_queue_id,
                satoshis,
                destination: PayoutDestination::OnchainAddress { value: destination },
                proportional_fee,
                ..
            } => proto::bria_event::Payload::PayoutCommitted(proto::PayoutCommitted {
                id: id.to_string(),
                tx_id: tx_id.to_string(),
                vout: vout.to_string(),
                wallet_id: wallet_id.to_string(),
                payout_queue_id: payout_queue_id.to_string(),
                satoshis: u64::from(satoshis),
                destination: Some(proto::payout_committed::Destination::OnchainAddress(
                    destination.to_string(),
                )),
                proportional_fee_sats: u64::from(proportional_fee),
            }),
            OutboxEventPayload::PayoutBroadcast {
                id,
                tx_id,
                vout,
                wallet_id,
                payout_queue_id,
                satoshis,
                destination: PayoutDestination::OnchainAddress { value: destination },
                proportional_fee,
                ..
            } => proto::bria_event::Payload::PayoutBroadcast(proto::PayoutBroadcast {
                id: id.to_string(),
                tx_id: tx_id.to_string(),
                vout: vout.to_string(),
                wallet_id: wallet_id.to_string(),
                payout_queue_id: payout_queue_id.to_string(),
                satoshis: u64::from(satoshis),
                destination: Some(proto::payout_broadcast::Destination::OnchainAddress(
                    destination.to_string(),
                )),
                proportional_fee_sats: u64::from(proportional_fee),
            }),
            OutboxEventPayload::PayoutSettled {
                id,
                tx_id,
                vout,
                wallet_id,
                payout_queue_id,
                satoshis,
                destination: PayoutDestination::OnchainAddress { value: destination },
                proportional_fee,
                ..
            } => proto::bria_event::Payload::PayoutSettled(proto::PayoutSettled {
                id: id.to_string(),
                tx_id: tx_id.to_string(),
                vout: vout.to_string(),
                wallet_id: wallet_id.to_string(),
                payout_queue_id: payout_queue_id.to_string(),
                satoshis: u64::from(satoshis),
                destination: Some(proto::payout_settled::Destination::OnchainAddress(
                    destination.to_string(),
                )),
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
        use crate::{
            address::error::*, payout::error::*, payout_queue::error::*, profile::error::*,
            wallet::error::*,
        };

        match err {
            ApplicationError::ProfileError(ProfileError::ProfileKeyNotFound) => {
                tonic::Status::unauthenticated(err.to_string())
            }
            ApplicationError::WalletError(WalletError::WalletNameNotFound(_)) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::WalletError(WalletError::WalletIdNotFound(_)) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::AddressError(AddressError::ExternalIdNotFound) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::AddressError(AddressError::ExternalIdAlreadyExists) => {
                tonic::Status::already_exists(err.to_string())
            }
            ApplicationError::PayoutQueueError(PayoutQueueError::PayoutQueueNameNotFound(_)) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::PayoutQueueError(PayoutQueueError::PayoutQueueIdNotFound(_)) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::ProfileError(ProfileError::ProfileNameNotFound(_)) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::PayoutError(PayoutError::PayoutIdNotFound(_)) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::CouldNotParseIncomingMetadata(_) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            ApplicationError::CouldNotParseIncomingUuid(_) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            ApplicationError::PayoutError(PayoutError::ExternalIdNotFound) => {
                tonic::Status::not_found(err.to_string())
            }
            ApplicationError::DestinationBlocked(_) => {
                tonic::Status::permission_denied(err.to_string())
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
            _ => tracing::Level::ERROR,
        }
    }
}
