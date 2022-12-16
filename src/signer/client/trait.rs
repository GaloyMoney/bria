use async_trait::async_trait;
use bitcoin::util::psbt::PartiallySignedTransaction;

use super::error::*;

#[async_trait]
pub trait RemoteSigningClient {
    async fn sign_psbt(
        &mut self,
        psbt: &PartiallySignedTransaction,
    ) -> Result<PartiallySignedTransaction, SigningClientError>;
}
