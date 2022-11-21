#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "KeychainKind", rename_all = "snake_case")]
pub enum KeychainKindPg {
    External,
    Internal,
}

impl From<bdk::KeychainKind> for KeychainKindPg {
    fn from(kind: bdk::KeychainKind) -> Self {
        match kind {
            bdk::KeychainKind::External => Self::External,
            bdk::KeychainKind::Internal => Self::Internal,
        }
    }
}
