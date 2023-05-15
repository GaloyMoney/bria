use rust_decimal::Decimal;
use sqlx_ledger::balance::AccountBalance;

use crate::primitives::{LedgerAccountId, Satoshis, WalletId};

const
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
        // get first n bytes of wallet id
        // encode in hex
        // insert in format! statement
        Self {
            onchain_incoming_id: format!("00000000-{IN_OUT_NUMBER}{FEE_NON_FEE}-0000-0000-{wallet_id_prefix}")
                .parse()
                .expect("invalid account id"),
            onchain_at_rest_id: format!("00000000-2010-0000-0000-000000000000")
                .parse()
                .expect("invalid account id"),
            onchain_outgoing_id: format!("00000000-2010-0000-0000-000000000000")
                .parse()
                .expect("invalid account id"),
            effective_at_rest_id: format!("00000000-2010-0000-0000-000000000000")
                .parse()
                .expect("invalid account id"),
            effective_outgoing_id: format!("00000000-2010-0000-0000-000000000000")
                .parse()
                .expect("invalid account id"),
            fee_id: format!("00000000-2010-0000-0000-000000000000")
                .parse()
                .expect("invalid account id"),
            dust_id: format!("00000000-2010-0000-0000-000000000000")
                .parse()
                .expect("invalid account id"),
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
