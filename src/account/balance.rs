use rust_decimal::Decimal;
use sqlx_ledger::balance::AccountBalance;

use crate::primitives::Satoshis;

#[derive(Debug)]
pub struct AccountLedgerAccountBalances {
    pub onchain_incoming: Option<AccountBalance>,
    pub onchain_at_rest: Option<AccountBalance>,
    pub onchain_outgoing: Option<AccountBalance>,
    pub effective_incoming: Option<AccountBalance>,
    pub effective_at_rest: Option<AccountBalance>,
    pub effective_outgoing: Option<AccountBalance>,
    pub fee: Option<AccountBalance>,
}

#[derive(Debug)]
pub struct AccountBalanceSummary {
    pub encumbered_incoming_utxos: Satoshis,
    pub pending_incoming_utxos: Satoshis,
    pub settled_utxos: Satoshis,
    pub pending_outgoing_utxos: Satoshis,
    pub pending_fees: Satoshis,
    pub encumbered_fees: Satoshis,
    pub effective_settled: Satoshis,
    pub effective_pending_income: Satoshis,
    pub effective_pending_outgoing: Satoshis,
    pub effective_encumbered_outgoing: Satoshis,
}

impl From<AccountLedgerAccountBalances> for AccountBalanceSummary {
    fn from(balances: AccountLedgerAccountBalances) -> Self {
        Self {
            encumbered_incoming_utxos: Satoshis::from_btc(
                balances
                    .onchain_incoming
                    .as_ref()
                    .map(|b| b.encumbered())
                    .unwrap_or(Decimal::ZERO),
            ),
            pending_incoming_utxos: Satoshis::from_btc(
                balances
                    .onchain_incoming
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            pending_outgoing_utxos: Satoshis::from_btc(
                balances
                    .onchain_outgoing
                    .as_ref()
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            settled_utxos: Satoshis::from_btc(
                balances
                    .onchain_at_rest
                    .map(|b| b.settled())
                    .unwrap_or(Decimal::ZERO),
            ),
            pending_fees: Satoshis::from_btc(
                balances
                    .fee
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
