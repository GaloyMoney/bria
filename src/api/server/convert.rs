use crate::error::BriaError;
use crate::signer::*;

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
                format!("missing signer config"),
            )),
        }
    }
}
