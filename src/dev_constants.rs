use crate::xpub::EncryptionKey;

pub const DEV_ACCOUNT_NAME: &str = "dev";
pub const BRIA_DEV_KEY: &str = "bria_dev_000000000000000000000";
const DEV_SIGNER_ENCRYPTION_KEY: &str = "00000000000000000000";

pub fn dev_signer_encryption_key() -> EncryptionKey {
    let key_vec = hex::decode(DEV_SIGNER_ENCRYPTION_KEY).unwrap();
    EncryptionKey::clone_from_slice(key_vec.as_ref())
}
