use crate::xpub::EncryptionKey;

pub const DEV_ACCOUNT_NAME: &str = "dev";
pub const BRIA_DEV_KEY: &str = "bria_dev_000000000000000000000";
const DEV_SIGNER_ENCRYPTION_KEY: &str =
    "5abe4753d810aa74f2ff9ec0f652017acb991b5b283253c742bf93bee4d3644e";

pub fn dev_signer_encryption_key() -> EncryptionKey {
    let key_vec = hex::decode(DEV_SIGNER_ENCRYPTION_KEY).unwrap();
    EncryptionKey::clone_from_slice(key_vec.as_ref())
}
