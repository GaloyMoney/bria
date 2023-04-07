use std::collections::HashMap;

use super::SpentUtxo;
use crate::primitives::*;

pub(super) fn withdraw_from_logical_when_settled(
    inputs: Vec<SpentUtxo>,
    change: Satoshis,
) -> (Satoshis, HashMap<bitcoin::OutPoint, Satoshis>) {
    let (total_in, settled_in) = inputs.iter().fold(
        (Satoshis::ZERO, Satoshis::ZERO),
        |(t, s),
         SpentUtxo {
             value, confirmed, ..
         }| (t + *value, if *confirmed { s + *value } else { s }),
    );
    let spent = total_in - change;
    if settled_in >= spent {
        return (
            settled_in,
            inputs
                .into_iter()
                .map(|SpentUtxo { outpoint, .. }| (outpoint, Satoshis::ZERO))
                .collect(),
        );
    }

    let already_deducted = if settled_in >= change {
        settled_in - change
    } else {
        Satoshis::ZERO
    };
    let mut needs_allocating = spent - already_deducted;
    let mut allocations = HashMap::new();
    for spent_utxo in inputs {
        if spent_utxo.confirmed || spent_utxo.change_address {
            allocations.insert(spent_utxo.outpoint, Satoshis::ZERO);
        } else {
            let amount = needs_allocating.min(spent_utxo.value);
            allocations.insert(spent_utxo.outpoint, amount);
            needs_allocating = Satoshis::ZERO.max(needs_allocating - amount);
        }
    }

    (settled_in, allocations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::OutPoint;

    #[test]
    fn test_confirmed_utxos_with_zero_change() {
        let txid = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let outpoint1 = OutPoint { txid, vout: 0 };

        let inputs = vec![SpentUtxo {
            outpoint: outpoint1.clone(),
            value: Satoshis::from(10000),
            change_address: false,
            confirmed: true,
        }];
        let change = Satoshis::ZERO;
        let (_, allocations) = withdraw_from_logical_when_settled(inputs, change);

        assert_eq!(*allocations.get(&outpoint1).unwrap(), Satoshis::ZERO);
    }

    #[test]
    fn test_unconfirmed_utxos() {
        let txid = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let outpoint1 = OutPoint { txid, vout: 0 };
        let outpoint2 = OutPoint { txid, vout: 1 };
        let outpoint3 = OutPoint { txid, vout: 2 };
        let one_btc = Satoshis::from(100_000_000);

        let inputs = vec![
            SpentUtxo {
                outpoint: outpoint1.clone(),
                value: Satoshis::from(40000),
                change_address: true,
                confirmed: true,
            },
            SpentUtxo {
                outpoint: outpoint2.clone(),
                value: one_btc,
                change_address: false,
                confirmed: false,
            },
            SpentUtxo {
                outpoint: outpoint3.clone(),
                value: one_btc,
                change_address: false,
                confirmed: false,
            },
        ];
        let change = Satoshis::from(30000);
        let (_, allocations) = withdraw_from_logical_when_settled(inputs, change);

        assert_eq!(*allocations.get(&outpoint1).unwrap(), Satoshis::ZERO);
        assert_eq!(*allocations.get(&outpoint2).unwrap(), one_btc);
        assert_eq!(*allocations.get(&outpoint3).unwrap(), one_btc);
    }
}
