use std::collections::{HashMap, HashSet};

use crate::primitives::{bitcoin::*, *};

pub struct CpfpUtxo {
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub value: Satoshis,
    pub additional_vbytes: u64,
    pub included_fees: Satoshis,
}

#[derive(Hash, PartialEq, Eq, Clone)]
pub(super) struct CpfpCandidate {
    pub utxo_history_tip: bool,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub ancestor_tx_id: Option<Txid>,
    pub origin_tx_vbytes: u64,
    pub origin_tx_fee: Satoshis,
}

pub(super) fn extract_cpfp_utxos(
    mut candidates: HashSet<CpfpCandidate>,
) -> HashMap<KeychainId, Vec<CpfpUtxo>> {
    let mut result = HashMap::new();
    loop {
        let utxo_history_tip = candidates.iter().find(|c| c.utxo_history_tip).cloned();
        if let Some(tip) = utxo_history_tip {
            candidates.remove(&tip);
            let mut additional_vbytes = tip.origin_tx_vbytes;
            let mut included_fees = tip.origin_tx_fee;
            let mut next_ancestor = tip.ancestor_tx_id;
            loop {
                if let Some(next_tx_id) = next_ancestor {
                    let ancestor = candidates
                        .iter()
                        .find(|c| c.outpoint.txid == next_tx_id)
                        .cloned();
                    if let Some(ancestor) = ancestor {
                        candidates.remove(&ancestor);
                        additional_vbytes += ancestor.origin_tx_vbytes;
                        included_fees += ancestor.origin_tx_fee;
                        next_ancestor = ancestor.ancestor_tx_id;
                        continue;
                    }
                }
                break;
            }
            let utxos: &mut Vec<_> = result.entry(tip.keychain_id).or_default();
            utxos.push(
                CpfpUtxo {
                    keychain_id: tip.keychain_id,
                    outpoint: tip.outpoint,
                    value: tip.origin_tx_fee,
                    additional_vbytes,
                    included_fees,
                }
                .into(),
            );
            continue;
        }
        break;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_without_ancestors() {
        let keychain_id1 = KeychainId::new();
        let keychain_id2 = KeychainId::new();
        let txid = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let candidate1 = CpfpCandidate {
            keychain_id: keychain_id1,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 0 },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let txid = "5010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let candidate2 = CpfpCandidate {
            keychain_id: keychain_id2,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 0 },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let txid = "5011e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let candidate3 = CpfpCandidate {
            keychain_id: keychain_id2,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 0 },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };

        let res = extract_cpfp_utxos(
            vec![candidate1, candidate2, candidate3]
                .into_iter()
                .collect(),
        );
        let utxo = res.get(&keychain_id1).unwrap();
        assert_eq!(utxo.len(), 1);
        let utxo = res.get(&keychain_id2).unwrap();
        assert_eq!(utxo.len(), 2);
    }

    #[test]
    fn accumalates_ancestors() {
        let keychain_id = KeychainId::new();
        let txid = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let ancestor_id = "5010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let candidate1 = CpfpCandidate {
            keychain_id,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 0 },
            ancestor_tx_id: Some(ancestor_id),
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let candidate2 = CpfpCandidate {
            keychain_id,
            utxo_history_tip: false,
            outpoint: OutPoint {
                txid: ancestor_id,
                vout: 0,
            },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };

        let res = extract_cpfp_utxos(vec![candidate1, candidate2].into_iter().collect());
        let utxos = res.get(&keychain_id).unwrap();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].additional_vbytes, 84);
        assert_eq!(utxos[0].included_fees, Satoshis::from(84));
    }
}
