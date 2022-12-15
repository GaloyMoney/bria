use async_trait::async_trait;
use bitcoin::{consensus::encode, util::psbt::PartiallySignedTransaction};
use serde::{Deserialize, Serialize};
use tonic_lnd::walletrpc::SignPsbtRequest;

use super::{error::*, r#trait::*};

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct LndRemoteSignerConfig {
    pub endpoint: String,
    pub cert_file: String,
    pub macaroon_file: String,
}

pub struct LndRemoteSigner {
    inner: tonic_lnd::Client,
}

impl LndRemoteSigner {
    pub async fn connect(cfg: LndRemoteSignerConfig) -> Result<Self, SigningClientError> {
        let client = tonic_lnd::connect(cfg.endpoint, cfg.cert_file, cfg.macaroon_file)
            .await
            .map_err(|e| {
                SigningClientError::CouldNotConnect(format!("Failed to connect to lnd: {}", e))
            })?;
        Ok(Self { inner: client })
    }
}

#[async_trait]
impl RemoteSigningClient for LndRemoteSigner {
    async fn sign_psbt(
        &mut self,
        psbt: &PartiallySignedTransaction,
    ) -> Result<PartiallySignedTransaction, SigningClientError> {
        let response = self
            .inner
            .wallet()
            .sign_psbt(SignPsbtRequest {
                funded_psbt: encode::serialize(psbt),
            })
            .await
            .map_err(|e| {
                SigningClientError::RemoteCallFailure(format!("Failed to sign psbt via lnd: {}", e))
            })?;
        Ok(encode::deserialize(&response.into_inner().signed_psbt)?)
    }
}
