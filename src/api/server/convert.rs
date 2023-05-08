use std::time::Duration;

use super::proto;
use crate::{
    account::balance::AccountBalanceSummary,
    address::*,
    batch_group::*,
    error::BriaError,
    outbox::*,
    payout::*,
    primitives::{bitcoin::*, *},
    profile::*,
    signing_session::*,
    utxo::*,
    wallet::balance::WalletBalanceSummary,
    wallet::*,
    xpub::*,
};

impl From<BriaError> for tonic::Status {
    fn from(err: BriaError) -> Self {
        match err {
            BriaError::CouldNotParseIncomingMetadata(err) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            BriaError::CouldNotParseIncomingUuid(err) => {
                tonic::Status::invalid_argument(err.to_string())
            }
            _ => tonic::Status::new(tonic::Code::Unknown, format!("{err}")),
        }
    }
}

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

impl TryFrom<Option<proto::queue_payout_request::Destination>> for PayoutDestination {
    type Error = tonic::Status;

    fn try_from(
        destination: Option<proto::queue_payout_request::Destination>,
    ) -> Result<Self, Self::Error> {
        match destination {
            Some(proto::queue_payout_request::Destination::OnchainAddress(destination)) => {
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
            batch_group_id: payout.batch_group_id.to_string(),
            batch_id: payout.batch_id.map(|id| id.to_string()),
            satoshis: u64::from(payout.satoshis),
            destination: Some(destination),
            external_id: payout.external_id,
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

impl From<BatchGroup> for proto::BatchGroup {
    fn from(batch_group: BatchGroup) -> Self {
        let id = batch_group.id.to_string();
        let name = batch_group.name;
        let consolidate_deprecated_keychains = batch_group.config.consolidate_deprecated_keychains;
        let trigger = match batch_group.config.trigger {
            BatchGroupTrigger::Manual => proto::batch_group_config::Trigger::Manual(true),
            BatchGroupTrigger::Immediate => proto::batch_group_config::Trigger::Immediate(true),
            BatchGroupTrigger::Interval { seconds } => {
                proto::batch_group_config::Trigger::IntervalSecs(seconds.as_secs() as u32)
            }
        };
        let tx_priority: proto::TxPriority = batch_group.config.tx_priority.into();
        let config = Some(proto::BatchGroupConfig {
            trigger: Some(trigger),
            tx_priority: tx_priority as i32,
            consolidate_deprecated_keychains,
        });
        proto::BatchGroup { id, name, config }
    }
}

impl From<TxPriority> for proto::TxPriority {
    fn from(priority: TxPriority) -> Self {
        match priority {
            TxPriority::NextBlock => proto::TxPriority::NextBlock,
            TxPriority::OneHour => proto::TxPriority::OneHour,
            TxPriority::Economy => proto::TxPriority::Economy,
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

impl From<proto::BatchGroupConfig> for BatchGroupConfig {
    fn from(proto_config: proto::BatchGroupConfig) -> Self {
        let tx_priority =
            proto::TxPriority::from_i32(proto_config.tx_priority).map(TxPriority::from);
        let consolidate_deprecated_keychains = proto_config.consolidate_deprecated_keychains;

        let trigger = match proto_config.trigger {
            Some(proto::batch_group_config::Trigger::Manual(_)) => Some(BatchGroupTrigger::Manual),
            Some(proto::batch_group_config::Trigger::Immediate(_)) => {
                Some(BatchGroupTrigger::Immediate)
            }
            Some(proto::batch_group_config::Trigger::IntervalSecs(interval)) => {
                Some(BatchGroupTrigger::Interval {
                    seconds: Duration::from_secs(interval as u64),
                })
            }
            None => None,
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
            proto::TxPriority::OneHour => TxPriority::OneHour,
            proto::TxPriority::Economy => TxPriority::Economy,
        }
    }
}

impl From<WalletBalanceSummary> for proto::GetWalletBalanceSummaryResponse {
    fn from(balance: WalletBalanceSummary) -> Self {
        Self {
            encumbered_incoming_utxos: u64::try_from(balance.encumbered_incoming_utxos)
                .expect("Satoshis -> u64 failed"),
            pending_incoming_utxos: u64::try_from(balance.pending_incoming_utxos)
                .expect("Satoshis -> u64 failed"),
            settled_utxos: u64::try_from(balance.settled_utxos).expect("Satoshis -> u64 failed"),
            pending_outgoing_utxos: u64::try_from(balance.pending_outgoing_utxos)
                .expect("Satoshis -> u64 failed"),
            pending_fees: u64::try_from(balance.pending_fees).expect("Satoshis -> u64 failed"),
            encumbered_fees: u64::try_from(balance.encumbered_fees)
                .expect("Satoshis -> u64 failed"),
            logical_pending_income: u64::try_from(balance.logical_pending_income)
                .expect("Satoshis -> u64 failed"),
            logical_settled: u64::try_from(balance.logical_settled)
                .expect("Satoshis -> u64 failed"),
            logical_pending_outgoing: u64::try_from(balance.logical_pending_outgoing)
                .expect("Satoshis -> u64 failed"),
            logical_encumbered_outgoing: u64::try_from(balance.logical_encumbered_outgoing)
                .expect("Satoshis -> u64 failed"),
        }
    }
}

impl From<AccountBalanceSummary> for proto::GetAccountBalanceSummaryResponse {
    fn from(balance: AccountBalanceSummary) -> Self {
        Self {
            encumbered_incoming_utxos: u64::try_from(balance.encumbered_incoming_utxos)
                .expect("Satoshis -> u64 failed"),
            pending_incoming_utxos: u64::try_from(balance.pending_incoming_utxos)
                .expect("Satoshis -> u64 failed"),
            settled_utxos: u64::try_from(balance.settled_utxos).expect("Satoshis -> u64 failed"),
            pending_outgoing_utxos: u64::try_from(balance.pending_outgoing_utxos)
                .expect("Satoshis -> u64 failed"),
            pending_fees: u64::try_from(balance.pending_fees).expect("Satoshis -> u64 failed"),
            encumbered_fees: u64::try_from(balance.encumbered_fees)
                .expect("Satoshis -> u64 failed"),
            logical_pending_income: u64::try_from(balance.logical_pending_income)
                .expect("Satoshis -> u64 failed"),
            logical_settled: u64::try_from(balance.logical_settled)
                .expect("Satoshis -> u64 failed"),
            logical_pending_outgoing: u64::try_from(balance.logical_pending_outgoing)
                .expect("Satoshis -> u64 failed"),
            logical_encumbered_outgoing: u64::try_from(balance.logical_encumbered_outgoing)
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
        };

        let augmentation = event.augmentation.map(|a| proto::EventAugmentation {
            address_info: a.address.map(proto::WalletAddress::from),
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
