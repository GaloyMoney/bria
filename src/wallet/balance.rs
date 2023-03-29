use rust_decimal::Decimal;
use sqlx_ledger::balance::AccountBalance;

use crate::primitives::{LedgerAccountId, Satoshis};

#[derive(Debug, Clone, Copy)]
pub struct WalletLedgerAccountIds {
    pub onchain_incoming_id: LedgerAccountId,
    pub onchain_at_rest_id: LedgerAccountId,
    pub onchain_outgoing_id: LedgerAccountId,
    pub logical_incoming_id: LedgerAccountId,
    pub logical_at_rest_id: LedgerAccountId,
    pub logical_outgoing_id: LedgerAccountId,
    pub fee_id: LedgerAccountId,
    pub dust_id: LedgerAccountId,
}

#[derive(Debug)]
pub struct WalletLedgerAccountBalances {
    pub onchain_incoming: Option<AccountBalance>,
    pub onchain_at_rest: Option<AccountBalance>,
    pub onchain_outgoing: Option<AccountBalance>,
    pub logical_incoming: Option<AccountBalance>,
    pub logical_at_rest: Option<AccountBalance>,
    pub logical_outgoing: Option<AccountBalance>,
    pub fee: Option<AccountBalance>,
    pub dust: Option<AccountBalance>,
}

#[derive(Debug)]
pub struct WalletBalanceSummary {
    pub confirmed_utxos: Satoshis,
    pub pending_incoming_utxos: Satoshis,
    pub pending_outgoing_utxos: Satoshis,
    pub pending_fees: Satoshis,
    pub encumbered_fees: Satoshis,
    pub logical_settled: Satoshis,
    pub logical_pending_income: Satoshis,
    pub logical_pending_outgoing: Satoshis,
    pub logical_encumbered_outgoing: Satoshis,
}

impl From<WalletLedgerAccountBalances> for WalletBalanceSummary {
    fn from(balances: WalletLedgerAccountBalances) -> Self {
        Self {
            confirmed_utxos: Satoshis::from_btc(
                balances
                    .onchain_at_rest
                    .map(|b| b.settled())
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
            logical_settled: Satoshis::from_btc(
                balances
                    .logical_at_rest
                    .map(|b| b.settled())
                    .unwrap_or(Decimal::ZERO),
            ),
            logical_pending_income: Satoshis::from_btc(
                balances
                    .logical_incoming
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            logical_pending_outgoing: Satoshis::from_btc(
                balances
                    .logical_outgoing
                    .as_ref()
                    .map(|b| b.pending())
                    .unwrap_or(Decimal::ZERO),
            ),
            logical_encumbered_outgoing: Satoshis::from_btc(
                balances
                    .onchain_outgoing
                    .map(|b| b.encumbered())
                    .unwrap_or(Decimal::ZERO),
            ),
        }
    }
}
