#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "BdkKeychainKind", rename_all = "snake_case")]
pub enum BdkKeychainKind {
    External,
    Internal,
}

impl From<bdk::KeychainKind> for BdkKeychainKind {
    fn from(kind: bdk::KeychainKind) -> Self {
        match kind {
            bdk::KeychainKind::External => Self::External,
            bdk::KeychainKind::Internal => Self::Internal,
        }
    }
}

impl From<BdkKeychainKind> for bdk::KeychainKind {
    fn from(kind: BdkKeychainKind) -> Self {
        match kind {
            BdkKeychainKind::External => Self::External,
            BdkKeychainKind::Internal => Self::Internal,
        }
    }
}
