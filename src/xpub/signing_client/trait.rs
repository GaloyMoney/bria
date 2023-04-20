use async_trait::async_trait;

use crate::primitives::bitcoin::psbt;

use super::error::*;

#[async_trait]
pub trait RemoteSigningClient: Send + 'static {
    async fn sign_psbt(
        &mut self,
        psbt: &psbt::PartiallySignedTransaction,
    ) -> Result<psbt::PartiallySignedTransaction, SigningClientError>;
}
