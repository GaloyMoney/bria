use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305,
};
use serde::{Deserialize, Serialize};

use super::signing_client::*;
use crate::error::*;

#[derive(Debug, Clone)]
pub struct SignerEncryptionConfig {
    #[serde(skip)]
    pub key: Option<EncryptionKey>,
}

mod key_serialization {
    use super::EncryptionKey;

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<EncryptionKey, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        unimplemented!()
    }

    pub(super) fn serialize<S>(key: &EncryptionKey, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        unimplemented!()
    }
}

pub type EncryptionKey = chacha20poly1305::Key;
pub(super) struct ConfigCyper(Vec<u8>);
pub(super) struct Nonce(Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SignerConfig {
    Lnd(LndSignerConfig),
    Bitcoind(BitcoindSignerConfig),
}

impl SignerConfig {
    pub(super) fn encrypt(&self, key: &EncryptionKey) -> Result<(ConfigCyper, Nonce), BriaError> {
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let encrypted_config = cipher
            .encrypt(&nonce, serde_json::to_vec(self)?.as_slice())
            .unwrap();

        Ok((ConfigCyper(encrypted_config), Nonce(nonce.to_vec())))
    }

    pub(super) fn decrypt(
        key: &EncryptionKey,
        encrypted_config: ConfigCyper,
        nonce: Nonce,
    ) -> Result<Self, BriaError> {
        let cipher = ChaCha20Poly1305::new(key);
        let decrypted_config = cipher
            .decrypt(
                &chacha20poly1305::Nonce::from_slice(nonce.0.as_slice()),
                encrypted_config.0.as_slice(),
            )
            .unwrap();
        let config: SignerConfig = serde_json::from_slice(decrypted_config.as_slice())?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    pub use super::*;

    fn gen_encryption_key() -> EncryptionKey {
        ChaCha20Poly1305::generate_key(&mut OsRng)
    }

    #[test]
    fn encrypt_decrypt() {
        let signer = SignerConfig::Lnd(LndSignerConfig {
            endpoint: "localhost".to_string(),
            cert_base64: "blabla".to_string(),
            macaroon_base64: "blabla".to_string(),
        });
        let key = gen_encryption_key();
        let (encrypted, nonce) = signer.encrypt(&key).expect("Failed to encrypt");
        let decrypted = SignerConfig::decrypt(&key, encrypted, nonce).expect("Failed to decrypt");

        assert_eq!(signer, decrypted);
    }
}
