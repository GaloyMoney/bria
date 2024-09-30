use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::{Deserialize, Serialize};

use super::{error::*, r#trait::*};
use crate::{primitives::bitcoin::psbt, wallet::DEFAULT_SIGHASH_TYPE};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitcoindSignerConfig {
    pub endpoint: String,
    pub rpc_user: String,
    pub rpc_password: String,
}

pub struct BitcoindRemoteSigner {
    inner: bitcoincore_rpc::Client,
}

impl BitcoindRemoteSigner {
    pub async fn connect(cfg: &BitcoindSignerConfig) -> Result<Self, SigningClientError> {
        let auth = Auth::UserPass(cfg.rpc_user.to_string(), cfg.rpc_password.to_string());
        let client = Client::new(&cfg.endpoint.to_string(), auth).map_err(|e| {
            SigningClientError::CouldNotConnect(format!(
                "Failed to connect to bitcoind @ {}: {e}",
                cfg.endpoint
            ))
        })?;

        Ok(Self { inner: client })
    }
}

#[async_trait]
impl RemoteSigningClient for BitcoindRemoteSigner {
    async fn sign_psbt(
        &mut self,
        psbt: &psbt::PartiallySignedTransaction,
    ) -> Result<psbt::PartiallySignedTransaction, SigningClientError> {
        let raw_psbt = psbt.serialize();
        let hex_psbt = general_purpose::STANDARD.encode(raw_psbt);
        let sighash_type = Some(DEFAULT_SIGHASH_TYPE.into());
        let response = self
            .inner
            .wallet_process_psbt(&hex_psbt, None, sighash_type, None)
            .map_err(|e| {
                SigningClientError::RemoteCallFailure(format!(
                    "Failed to sign psbt via bitcoind: {e}"
                ))
            })?;
        let signed_psbt = general_purpose::STANDARD
            .decode(response.psbt)
            .map_err(|e| {
                SigningClientError::HexConvert(format!("Failed to convert psbt from bitcoind: {e}"))
            })?;
        let deserialized_psbt = psbt::PartiallySignedTransaction::deserialize(&signed_psbt)?;
        dbg!(&deserialized_psbt);
        Ok(deserialized_psbt)
    }
}
