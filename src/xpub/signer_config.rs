use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Key, Nonce,
};
use serde::{Deserialize, Serialize};

use super::signing_client::*;
use crate::error::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignerConfig {
    Lnd(LndSignerConfig),
    Bitcoind(BitcoindSignerConfig),
}

impl SignerConfig {
    pub fn encrypt(&self, secret: String) -> Result<(Vec<u8>, Vec<u8>), BriaError> {
        let key_vec = hex::decode(secret).expect("Failed to decode Key");
        let key_slice = key_vec.as_slice();
        let key = Key::from_slice(key_slice);
        let cipher = ChaCha20Poly1305::new(&key);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let encrypted_config = cipher
            .encrypt(&nonce, serde_json::to_vec(self)?.as_slice())
            .unwrap();

        Ok((encrypted_config, nonce.to_vec()))
    }

    pub fn decrypt(
        secret: String,
        nonce: Vec<u8>,
        encrypted_config: Vec<u8>,
    ) -> Result<Self, BriaError> {
        let key_vec = hex::decode(secret).expect("Failed to decode Key");
        let key_slice = key_vec.as_slice();
        let key = Key::from_slice(key_slice);
        let cipher = ChaCha20Poly1305::new(&key);
        let decrypted_config = cipher
            .decrypt(
                &Nonce::from_slice(nonce.as_slice()),
                encrypted_config.as_slice(),
            )
            .unwrap();
        let config: SignerConfig = serde_json::from_slice(decrypted_config.as_slice())?;
        Ok(config)
    }
}
