use rust_decimal::Decimal;

pub struct WalletBalanceSummary {
    pub current_settled: Decimal,
    pub reserved_for_fees: Decimal,
    pub pending_income: Decimal,
    pub queued_outgoing: Decimal,
}
