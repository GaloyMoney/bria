use async_trait::async_trait;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use serde::{Deserialize, Serialize};

use super::{error::*, r#trait::*};
use crate::primitives::bitcoin::{consensus, hex::FromHex, psbt};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            SigningClientError::CouldNotConnect(format!("Failed to connect to bitcoind: {e}"))
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
        let raw_psbt = consensus::encode::serialize(&psbt);
        let hex_psbt = base64::encode(raw_psbt);

        let response = self
            .inner
            .wallet_process_psbt(&hex_psbt, None, None, None)
            .map_err(|e| {
                SigningClientError::RemoteCallFailure(format!(
                    "Failed to sign psbt via bitcoind: {e}"
                ))
            })?;

        let signed_psbt = Vec::<u8>::from_hex(&response.psbt).map_err(|e| {
            SigningClientError::HexConvert(format!("Failed to convert psbt from bitcoind: {e}"))
        })?;
        Ok(consensus::encode::deserialize(&signed_psbt)?)
    }
}
