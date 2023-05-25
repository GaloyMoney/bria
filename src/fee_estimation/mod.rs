mod electrum;
mod mempool_space;

pub use electrum::ElectrumFeeEstimator;
pub use mempool_space::*;

use crate::primitives::*;
use bdk::bitcoin::{LockTime, Transaction, TxOut};

/// Txin "base" fields include `outpoint` (32+4) and `nSequence` (4). This does not include
/// `scriptSigLen` or `scriptSig`.
const TXIN_BASE_WEIGHT: usize = (32 + 4 + 4) * 4;

pub fn estimate_proportional_fee(
    n_inputs: usize,
    input_satisfaction_weight: usize,
    fee_rate: bitcoin::FeeRate,
    avg_n_payouts: usize,
    avg_payout_value: Satoshis,
    output_destination: bitcoin::Address,
    output_value: Satoshis,
) -> Satoshis {
    let mut total_out = Satoshis::ZERO;
    let mut output = Vec::new();
    for _ in 0..avg_n_payouts {
        total_out += avg_payout_value;
        output.push(TxOut {
            value: u64::from(avg_payout_value),
            script_pubkey: output_destination.script_pubkey(),
        });
    }
    total_out += output_value;
    output.push(TxOut {
        value: u64::from(output_value),
        script_pubkey: output_destination.script_pubkey(),
    });
    // Simulate change utxo
    if avg_n_payouts == 0 {
        output.push(TxOut {
            value: 1,
            script_pubkey: output_destination.script_pubkey(),
        });
    }
    let tx = Transaction {
        input: vec![],
        version: 1,
        lock_time: LockTime::ZERO.into(),
        output,
    };

    let input_weight = (TXIN_BASE_WEIGHT + input_satisfaction_weight) * n_inputs;
    let total_weight = tx.weight() + input_weight + 2; // 2 for segwit marker and flag
    let fee = rust_decimal::Decimal::from(fee_rate.fee_wu(total_weight));
    let proportion = output_value.into_inner() / total_out.into_inner();
    let proportional_fee = fee * proportion;
    Satoshis::from(
        proportional_fee.round_dp_with_strategy(0, rust_decimal::RoundingStrategy::AwayFromZero),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_calculation() {
        // Reference tx https://blockstream.info/tx/c6260b24a8234f7cb6bd0698634d9056c1a3927a89ab5f98c0dcba199198f187
        // has 1 input and 2 outputs
        let bytes = hex::decode("01000000000101e4b803c2d1bbc799050ef212b6749b925e35e9530839c833aca4964c4278a3e4010000000080e3ffff02b738010000000000160014c3dc650ba285d0b7bcb0486ec7454e434146f6e093f6c20000000000160014c789c7a2800fdad9a330373a3b58319f4b7b0f8802483045022100a9dbe84dd0ce75aeac6bc9151e3ecea0d9be70ce93645d179bc61ca96bfd6eaa02200fa8facea14e00d207b830a0b0b3bb106a6735a4a8e0702232aa22c8ffa6a4e101210226f3fc10d64822765964345fd6bc71d48782d2c44bcef826089d0e4d709532ac00000000").unwrap();
        let tx: Transaction = bitcoin::consensus::encode::deserialize(&bytes).unwrap();
        assert_eq!(tx.weight(), 562);

        let fee_rate = bitcoin::FeeRate::from_sat_per_vb(1000.);

        let total_fee = Satoshis::from(fee_rate.fee_wu(tx.weight()));

        let descriptor : bdk::descriptor::ExtendedDescriptor = "wpkh([6f2fa1b2/84'/0'/0']tpubDDDDGYiFda8HfJRc2AHFJDxVzzEtBPrKsbh35EaW2UGd5qfzrF2G87ewAgeeRyHEz4iB3kvhAYW1sH6dpLepTkFUzAktumBN8AXeXWE9nd1/0/*)#l6n08zmr".parse().unwrap();
        let address = "bc1qc7yu0g5qplddngesxuarkkp3na9hkrugpydqs0"
            .parse()
            .unwrap();

        let estimate = estimate_proportional_fee(
            1,
            descriptor.max_satisfaction_weight().unwrap(),
            fee_rate,
            0,
            Satoshis::ZERO,
            address,
            Satoshis::from(127_000_000),
        );

        assert_eq!(estimate, total_fee);
    }
}
