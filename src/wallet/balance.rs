use rust_decimal::Decimal;
use sqlx_ledger::balance::AccountBalance;
use uuid::Uuid;

use crate::primitives::{LedgerAccountId, Satoshis, WalletId};

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
    wallet_id_suffix: &str,
) -> String {
    format!(
        "{}-{}{}{}-{}-{}-{}",
        CURRENCY_CODE,
        element.code(),
        HOT_WALLET_CODE,
        sub_group.code(),
        RESERVED,
        other.code(),
        wallet_id_suffix
    )
}

#[derive(Debug, Clone, Copy)]
pub struct WalletLedgerAccountIds {
    pub onchain_incoming_id: LedgerAccountId,
    pub onchain_at_rest_id: LedgerAccountId,
    pub onchain_outgoing_id: LedgerAccountId,
    pub effective_incoming_id: LedgerAccountId,
    pub effective_at_rest_id: LedgerAccountId,
    pub effective_outgoing_id: LedgerAccountId,
    pub fee_id: LedgerAccountId,
    pub dust_id: LedgerAccountId,
}

impl From<WalletId> for WalletLedgerAccountIds {
    fn from(wallet_id: WalletId) -> Self {
        let wallet_id_str = wallet_id.to_string();
        let wallet_id_suffix = &wallet_id_str[24..];
        let onchain_incoming_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::Incoming,
                Other::Onchain,
                wallet_id_suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet Id");

        let onchain_at_rest_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::AtRest,
                Other::Onchain,
                wallet_id_suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let onchain_outgoing_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::Outgoing,
                Other::Onchain,
                wallet_id_suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let effective_incoming_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::Incoming,
                Other::Effective,
                wallet_id_suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let effective_at_rest_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::AtRest,
                Other::Effective,
                wallet_id_suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let effective_outgoing_id = Uuid::parse_str(
            derive_complete_code(
                Element::Liability,
                SubGroup::Outgoing,
                Other::Effective,
                wallet_id_suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let fee_id = Uuid::parse_str(
            derive_complete_code(
                Element::Revenue,
                SubGroup::AtRest,
                Other::Fee,
                wallet_id_suffix,
            )
            .as_str(),
        )
        .expect("Invalid Wallet_Id");

        let dust_id = Uuid::parse_str(
            derive_complete_code(
                Element::Revenue,
                SubGroup::AtRest,
                Other::Dust,
                wallet_id_suffix,
            )
            .as_str(),
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

#[derive(Debug)]
pub struct WalletLedgerAccountBalances {
    pub onchain_incoming: Option<AccountBalance>,
    pub onchain_at_rest: Option<AccountBalance>,
    pub onchain_outgoing: Option<AccountBalance>,
    pub effective_incoming: Option<AccountBalance>,
    pub effective_at_rest: Option<AccountBalance>,
    pub effective_outgoing: Option<AccountBalance>,
    pub fee: Option<AccountBalance>,
    pub dust: Option<AccountBalance>,
}

#[derive(Debug)]
pub struct WalletBalanceSummary {
    pub utxo_encumbered_incoming: Satoshis,
    pub utxo_pending_incoming: Satoshis,
    pub utxo_settled: Satoshis,
    pub utxo_pending_outgoing: Satoshis,
    pub fees_pending: Satoshis,
    pub fees_encumbered: Satoshis,
    pub effective_settled: Satoshis,
    pub effective_pending_income: Satoshis,
    pub effective_pending_outgoing: Satoshis,
    pub effective_encumbered_outgoing: Satoshis,
}

impl From<WalletLedgerAccountBalances> for WalletBalanceSummary {
    fn from(balances: WalletLedgerAccountBalances) -> Self {
        Self {
            utxo_encumbered_incoming: Satoshis::from_btc(
                balances
                    .onchain_incoming
                    .as_ref()
                    .map(|b| b.encumbered())
                    .unwrap_or(Decimal::ZERO),
            ),
            utxo_pending_incoming: Satoshis::from_btc(
                balances
                    .onchain_incoming
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            utxo_pending_outgoing: Satoshis::from_btc(
                balances
                    .onchain_outgoing
                    .as_ref()
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            utxo_settled: Satoshis::from_btc(
                balances
                    .onchain_at_rest
                    .map(|b| b.settled())
                    .unwrap_or(Decimal::ZERO),
            ),
            fees_pending: Satoshis::from_btc(
                balances
                    .fee
                    .as_ref()
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            fees_encumbered: Satoshis::from_btc(
                balances
                    .fee
                    .map(|b| b.encumbered())
                    .unwrap_or(Decimal::ZERO),
            ),
            effective_settled: Satoshis::from_btc(
                balances
                    .effective_at_rest
                    .map(|b| b.settled())
                    .unwrap_or(Decimal::ZERO),
            ),
            effective_pending_income: Satoshis::from_btc(
                balances
                    .effective_incoming
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            effective_pending_outgoing: Satoshis::from_btc(
                balances
                    .effective_outgoing
                    .as_ref()
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            effective_encumbered_outgoing: Satoshis::from_btc(
                balances
                    .effective_outgoing
                    .map(|b| b.encumbered())
                    .unwrap_or(Decimal::ZERO),
            ),
        }
    }
}
