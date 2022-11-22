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
