use rust_decimal::Decimal;
use sqlx_ledger::balance::AccountBalance;

use crate::primitives::{LedgerAccountId, Satoshis};

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug)]
pub struct WalletBalanceSummary {
    pub current_settled: Satoshis,
    pub pending_incoming: Satoshis,
    pub pending_outgoing: Satoshis,
    pub encumbered_fees: Satoshis,
    pub encumbered_outgoing: Satoshis,
}

impl From<WalletLedgerAccountBalances> for WalletBalanceSummary {
    fn from(balances: WalletLedgerAccountBalances) -> Self {
        Self {
            current_settled: Satoshis::from_btc(
                balances
                    .at_rest
                    .map(|b| b.settled())
                    .unwrap_or(Decimal::ZERO),
            ),
            pending_incoming: Satoshis::from_btc(
                balances
                    .incoming
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            pending_outgoing: Satoshis::from_btc(
                balances
                    .outgoing
                    .as_ref()
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            encumbered_fees: Satoshis::from_btc(
                balances
                    .fee
                    .map(|b| b.encumbered())
                    .unwrap_or(Decimal::ZERO),
            ),
            encumbered_outgoing: Satoshis::from_btc(
                balances
                    .outgoing
                    .map(|b| b.encumbered())
                    .unwrap_or(Decimal::ZERO),
            ),
        }
    }
}
