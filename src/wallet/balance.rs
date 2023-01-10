use rust_decimal::Decimal;
use sqlx_ledger::{balance::AccountBalance, AccountId as LedgerAccountId};

use crate::primitives::SATS_PER_BTC;

#[derive(Debug, Clone)]
pub struct WalletLedgerAccountIds {
    pub incoming_id: LedgerAccountId,
    pub at_rest_id: LedgerAccountId,
    pub fee_id: LedgerAccountId,
    pub outgoing_id: LedgerAccountId,
    pub dust_id: LedgerAccountId,
}

#[derive(Debug)]
pub struct WalletLedgerAccountBalances {
    pub incoming: Option<AccountBalance>,
    pub at_rest: Option<AccountBalance>,
    pub fee: Option<AccountBalance>,
    pub outgoing: Option<AccountBalance>,
    pub dust: Option<AccountBalance>,
}

pub struct WalletBalanceSummary {
    pub current_settled: Decimal,
    pub pending_incoming: Decimal,
    pub pending_outgoing: Decimal,
    pub encumbered_fees: Decimal,
    pub encumbered_outgoing: Decimal,
}

impl From<WalletLedgerAccountBalances> for WalletBalanceSummary {
    fn from(balances: WalletLedgerAccountBalances) -> Self {
        Self {
            current_settled: balances
                .at_rest
                .map(|b| b.settled())
                .unwrap_or(Decimal::ZERO)
                * SATS_PER_BTC,
            pending_incoming: balances
                .incoming
                .map(|b| b.pending())
                .unwrap_or(Decimal::ZERO)
                * SATS_PER_BTC,
            pending_outgoing: balances
                .outgoing
                .as_ref()
                .map(|b| b.pending())
                .unwrap_or(Decimal::ZERO)
                * SATS_PER_BTC,
            encumbered_fees: balances
                .fee
                .map(|b| -b.encumbered())
                .unwrap_or(Decimal::ZERO)
                * SATS_PER_BTC,
            encumbered_outgoing: balances
                .outgoing
                .map(|b| b.encumbered())
                .unwrap_or(Decimal::ZERO)
                * SATS_PER_BTC,
        }
    }
}
