use super::signing_client::*;
use crate::error::*;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305,
};
use serde::{Deserialize, Serialize};

pub type EncryptionKey = chacha20poly1305::Key;
pub struct ConfigCyper(pub Vec<u8>);
pub struct Nonce(pub Vec<u8>);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(into = "RawSignerEncryptionConfig")]
#[serde(try_from = "RawSignerEncryptionConfig")]
pub struct SignerEncryptionConfig {
    pub key: EncryptionKey,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        encrypted_config: &ConfigCyper,
        nonce: &Nonce,
    ) -> Result<Self, BriaError> {
        let cipher = ChaCha20Poly1305::new(key);
        let decrypted_config = cipher
            .decrypt(
                chacha20poly1305::Nonce::from_slice(nonce.0.as_slice()),
                encrypted_config.0.as_slice(),
            )
            .unwrap();
        let config: SignerConfig = serde_json::from_slice(decrypted_config.as_slice())?;
        Ok(config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
struct RawSignerEncryptionConfig {
    pub key: String,
}
impl From<SignerEncryptionConfig> for RawSignerEncryptionConfig {
    fn from(config: SignerEncryptionConfig) -> Self {
        Self {
            key: hex::encode(config.key),
        }
    }
}

impl TryFrom<RawSignerEncryptionConfig> for SignerEncryptionConfig {
    type Error = BriaError;

    fn try_from(raw: RawSignerEncryptionConfig) -> Result<Self, Self::Error> {
        let key_vec = hex::decode(raw.key)?;
        let key_bytes = key_vec.as_slice();
        Ok(Self {
            key: EncryptionKey::clone_from_slice(key_bytes),
        })
    }
}

impl std::fmt::Debug for SignerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignerConfig::Lnd(config) => {
                write!(f, "SignerConfig::Lnd(endpoint={})", config.endpoint)
            }
            SignerConfig::Bitcoind(config) => {
                write!(f, "SignerConfig::Bitcoind(endpoint={})", config.endpoint)
            }
        }
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
        let decrypted = SignerConfig::decrypt(&key, &encrypted, &nonce).expect("Failed to decrypt");

        assert_eq!(signer, decrypted);
    }

    #[test]
    fn serialize_deserialize() {
        let key = gen_encryption_key();
        let signer_encryption_config = SignerEncryptionConfig { key };
        let serialized = serde_json::to_string(&signer_encryption_config).unwrap();
        let deserialized: SignerEncryptionConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.key, key);
        assert_eq!(signer_encryption_config, deserialized)
    }
}
