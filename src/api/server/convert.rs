use crate::error::BriaError;
use crate::payout::*;
use crate::xpub::*;

impl From<BriaError> for tonic::Status {
    fn from(err: BriaError) -> Self {
        tonic::Status::new(tonic::Code::Unknown, format!("{}", err))
    }
}

impl TryFrom<Option<super::proto::set_signer_config_request::Config>> for SignerConfig {
    type Error = tonic::Status;

    fn try_from(
        config: Option<super::proto::set_signer_config_request::Config>,
    ) -> Result<Self, Self::Error> {
        match config {
            Some(super::proto::set_signer_config_request::Config::Lnd(config)) => {
                Ok(SignerConfig::Lnd(LndSignerConfig {
                    endpoint: config.endpoint,
                    cert_base64: config.cert_base64,
                    macaroon_base64: config.macaroon_base64,
                }))
            }
            None => Err(tonic::Status::new(
                tonic::Code::InvalidArgument,
                "missing signer config",
            )),
        }
    }
}

impl TryFrom<Option<super::proto::queue_payout_request::Destination>> for PayoutDestination {
    type Error = tonic::Status;

    fn try_from(
        destination: Option<super::proto::queue_payout_request::Destination>,
    ) -> Result<Self, Self::Error> {
        match destination {
            Some(super::proto::queue_payout_request::Destination::OnchainAddress(destination)) => {
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
