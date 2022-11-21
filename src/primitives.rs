use bitcoin::util::bip32::Fingerprint;

crate::entity_id! { AdminApiKeyId }
crate::entity_id! { AccountId }
crate::entity_id! { AccountApiKeyId }
crate::entity_id! { WalletId }
crate::entity_id! { KeychainId }

pub struct XPubId(Fingerprint);

impl From<Fingerprint> for XPubId {
    fn from(fp: Fingerprint) -> Self {
        Self(fp)
    }
}

impl std::ops::Deref for XPubId {
    type Target = Fingerprint;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
