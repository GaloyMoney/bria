use crate::primitives::{LedgerAccountId, WalletId};
use crate::wallet::WalletLedgerAccountIds;
use uuid::Uuid;

const CURRENCY_CODE: &str = "00000000";
#[allow(dead_code)]
enum Element {
    Asset,
    Liability,
    Revenue,
    Expense,
}

impl Element {
    fn code(&self) -> &'static str {
        match self {
            Self::Asset => "1",
            Self::Liability => "2",
            Self::Revenue => "4",
            Self::Expense => "6",
        }
    }
}

const HOT_WALLET_CODE: &str = "0";

enum SubGroup {
    AtRest,
    Incoming,
    Outgoing,
}

impl SubGroup {
    fn code(&self) -> &'static str {
        match self {
            SubGroup::AtRest => "00",
            SubGroup::Incoming => "10",
            SubGroup::Outgoing => "20",
        }
    }
}

const RESERVED: &str = "0000";

enum Other {
    Onchain,
    Effective,
    Fee,
    Dust,
}

impl Other {
    fn code(&self) -> &'static str {
        match self {
            Other::Onchain => "0000",
            Other::Effective => "0001",
            Other::Fee => "0002",
            Other::Dust => "0003",
        }
    }
}

fn derive_complete_code(
    element: Element,
    sub_group: SubGroup,
    other: Other,
    suffix: &str,
) -> String {
    format!(
        "{}-{}{}{}-{}-{}-{}",
        CURRENCY_CODE,
        element.code(),
        HOT_WALLET_CODE,
        sub_group.code(),
        RESERVED,
        other.code(),
        suffix
    )
}

impl From<WalletId> for WalletLedgerAccountIds {
    fn from(wallet_id: WalletId) -> Self {
        let wallet_id_str = wallet_id.to_string();
        let wallet_id_without_hyphens = wallet_id_str.replace('-', "");
        let suffix = &wallet_id_without_hyphens[0..12];

        let onchain_incoming_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::Incoming,
                Other::Onchain,
                suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet Id");

        let onchain_at_rest_id = Uuid::parse_str(
            derive_complete_code(Element::Liability, SubGroup::AtRest, Other::Onchain, suffix)
                .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let onchain_outgoing_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::Outgoing,
                Other::Onchain,
                suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let effective_incoming_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::Incoming,
                Other::Effective,
                suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let effective_at_rest_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::AtRest,
                Other::Effective,
                suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let effective_outgoing_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::Outgoing,
                Other::Effective,
                suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let fee_id = Uuid::parse_str(
            derive_complete_code(Element::Revenue, SubGroup::AtRest, Other::Fee, suffix).as_str(),
        )
        .expect("Invalid Wallet_Id");

        let dust_id = Uuid::parse_str(
            derive_complete_code(Element::Revenue, SubGroup::AtRest, Other::Dust, suffix).as_str(),
        )
        .expect("Invalid Wallet_Id");

        Self {
            onchain_incoming_id: LedgerAccountId::from(onchain_incoming_id),
            onchain_at_rest_id: LedgerAccountId::from(onchain_at_rest_id),
            onchain_outgoing_id: LedgerAccountId::from(onchain_outgoing_id),
            effective_incoming_id: LedgerAccountId::from(effective_incoming_id),
            effective_at_rest_id: LedgerAccountId::from(effective_at_rest_id),
            effective_outgoing_id: LedgerAccountId::from(effective_outgoing_id),
            fee_id: LedgerAccountId::from(fee_id),
            dust_id: LedgerAccountId::from(dust_id),
        }
    }
}
