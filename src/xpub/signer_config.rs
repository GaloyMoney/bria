use super::signing_client::*;
use crate::error::*;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305,
};
use serde::{Deserialize, Serialize};

pub type EncryptionKey = chacha20poly1305::Key;
pub(super) struct ConfigCyper(Vec<u8>);
pub(super) struct Nonce(Vec<u8>);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignerEncryptionConfig {
    #[serde(with = "key_serialization")]
    pub key: Option<EncryptionKey>,
}

mod key_serialization {
    use super::EncryptionKey;
    use serde::de::{Error, Unexpected};
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Option<EncryptionKey>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex_string = String::deserialize(deserializer)?;
        let key_vec = hex::decode(&hex_string).map_err(|_err| {
            D::Error::invalid_value(Unexpected::Str(&hex_string), &"valid hex string")
        })?;

        let key_bytes = key_vec.as_slice();
        if key_bytes.len() == 32 {
            Ok(Some(*chacha20poly1305::Key::from_slice(key_bytes)))
        } else {
            Err(D::Error::invalid_length(key_bytes.len(), &"32-byte array"))
        }
    }

    pub(super) fn serialize<S>(
        key: &Option<EncryptionKey>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match key {
            Some(key) => {
                let key_bytes = key.as_slice();
                let hex_string = hex::encode(key_bytes);
                serializer.serialize_str(&hex_string)
            }
            None => serializer.serialize_none(),
        }
    }
}

impl Default for SignerEncryptionConfig {
    fn default() -> Self {
        Self { key: None }
    }
}

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

    #[test]
    fn serialize_deserialize() {
        let key = gen_encryption_key();
        let signer_encryption_config = SignerEncryptionConfig { key: Some(key) };
        let serialized = serde_json::to_string(&signer_encryption_config).unwrap();
        let deserialized: SignerEncryptionConfig = serde_json::from_str(&serialized).unwrap();
        assert!(deserialized.key.is_some());
        assert_eq!(deserialized.key.unwrap(), key);
        assert_eq!(signer_encryption_config, deserialized)
    }
}
