use std::collections::{HashMap, HashSet};

use crate::primitives::{bitcoin::*, *};

#[derive(Hash, PartialEq, Eq, Clone)]
pub(super) struct CpfpCandidate {
    pub utxo_history_tip: bool,
    pub keychain_id: KeychainId,
    pub outpoint: OutPoint,
    pub origin_tx_batch_id: Option<BatchId>,
    pub origin_tx_vbytes: u64,
    pub origin_tx_fee: Satoshis,
    pub ancestor_tx_id: Option<Txid>,
}

#[derive(Debug)]
pub struct FeeWeightAttribution {
    pub batch_id: Option<BatchId>,
    pub tx_id: bitcoin::Txid,
    pub fee: Satoshis,
    pub vbytes: u64,
}
#[derive(Debug)]
pub struct CpfpUtxo {
    pub keychain_id: KeychainId,
    pub outpoint: bitcoin::OutPoint,
    pub value: Satoshis,
    pub attributions: std::collections::HashMap<bitcoin::Txid, FeeWeightAttribution>,
}

impl CpfpUtxo {
    pub fn additional_vbytes(&self) -> u64 {
        self.attributions.values().fold(0, |acc, a| acc + a.vbytes)
    }

    pub fn included_fees(&self) -> Satoshis {
        self.attributions
            .values()
            .fold(Satoshis::ZERO, |acc, a| acc + a.fee)
    }

    pub fn missing_fees(
        &self,
        fee_rate: &bdk::FeeRate,
    ) -> HashMap<Txid, (Option<BatchId>, Satoshis)> {
        let total_vbytes = self.additional_vbytes();
        let included_fees = self.included_fees();
        let required_fee = fee_rate.fee_vb(total_vbytes as usize);
        let mut fees_still_to_allocate = Satoshis::from(required_fee) - included_fees;
        if fees_still_to_allocate <= Satoshis::ZERO {
            return HashMap::new();
        }
        let missing_fees = u64::from(fees_still_to_allocate) as f64;
        let len = self.attributions.len();

        self.attributions.iter().enumerate().fold(
            HashMap::new(),
            |mut acc, (idx, (tx_id, attr))| {
                if idx == len - 1 {
                    acc.insert(*tx_id, (attr.batch_id, fees_still_to_allocate));
                } else {
                    let fees_to_attribute =
                        (attr.vbytes as f64 * missing_fees) / total_vbytes as f64;
                    let sats_to_attribute = Satoshis::from(fees_to_attribute.round() as u64);
                    fees_still_to_allocate -= sats_to_attribute;
                    acc.insert(*tx_id, (attr.batch_id, sats_to_attribute));
                }
                acc
            },
        )
    }
}

pub(super) fn extract_cpfp_utxos(
    mut candidates: HashSet<CpfpCandidate>,
) -> HashMap<KeychainId, Vec<CpfpUtxo>> {
    let mut result = HashMap::new();
    loop {
        let utxo_history_tip = candidates.iter().find(|c| c.utxo_history_tip).cloned();
        if let Some(tip) = utxo_history_tip {
            let mut tx_ids = HashSet::new();
            let mut attributions = HashMap::new();
            tx_ids.insert(tip.outpoint.txid);
            let mut to_remove = HashSet::new();
            let mut added = true;
            while added {
                added = false;
                for candidate in candidates.iter() {
                    if tx_ids.contains(&candidate.outpoint.txid) {
                        attributions.insert(
                            candidate.outpoint.txid,
                            FeeWeightAttribution {
                                batch_id: candidate.origin_tx_batch_id,
                                tx_id: candidate.outpoint.txid,
                                fee: candidate.origin_tx_fee,
                                vbytes: candidate.origin_tx_vbytes,
                            },
                        );
                        to_remove.insert((candidate.outpoint.txid, candidate.ancestor_tx_id));
                        if let Some(tx_id) = candidate.ancestor_tx_id {
                            added = tx_ids.insert(tx_id) || added;
                        }
                    }
                }
                candidates.retain(|t| !to_remove.contains(&(t.outpoint.txid, t.ancestor_tx_id)));
                to_remove.clear();
            }

            candidates.retain(|c| !tx_ids.contains(&c.outpoint.txid));

            let utxos: &mut Vec<_> = result.entry(tip.keychain_id).or_default();
            utxos.push(CpfpUtxo {
                keychain_id: tip.keychain_id,
                outpoint: tip.outpoint,
                value: tip.origin_tx_fee,
                attributions,
            });
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
        let txid1 = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let candidate1 = CpfpCandidate {
            keychain_id: keychain_id1,
            origin_tx_batch_id: None,
            utxo_history_tip: true,
            outpoint: OutPoint {
                txid: txid1,
                vout: 0,
            },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let txid = "5010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let candidate2 = CpfpCandidate {
            keychain_id: keychain_id2,
            origin_tx_batch_id: None,
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
            origin_tx_batch_id: None,
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
        let utxos = res.get(&keychain_id1).unwrap();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].attributions.len(), 1);
        assert_eq!(
            utxos[0]
                .attributions
                .get(&txid1)
                .expect("missing attribution")
                .fee,
            Satoshis::from(42)
        );
        let missing_fees = utxos[0].missing_fees(&FeeRate::from_sat_per_vb(2.0));
        assert_eq!(missing_fees.len(), 1);
        let (_, missing_fee) = missing_fees
            .get(&txid1)
            .expect("missing missing fee for txid");
        assert_eq!(*missing_fee, Satoshis::from(42));
        let utxos = res.get(&keychain_id2).unwrap();
        assert_eq!(utxos.len(), 2);
    }

    #[test]
    fn accumulates_ancestors() {
        let keychain_id = KeychainId::new();
        let txid = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let txid2 = "4011e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let ancestor_id = "5010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();

        // Candidate with 1 unconfirmed ancestor
        let candidate1 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: None,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 0 },
            ancestor_tx_id: Some(ancestor_id),
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let ancestor_batch_id = BatchId::new();
        let ancestor1 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: Some(ancestor_batch_id),
            utxo_history_tip: false,
            outpoint: OutPoint {
                txid: ancestor_id,
                vout: 0,
            },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };

        // Candidate in same tx as candidate1 (should be ignored)
        let candidate2 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: None,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 1 },
            ancestor_tx_id: Some(ancestor_id),
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };

        // Candidate with same ancestor as candidate 1 (should be included but not rolled up)
        let candidate3 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: None,
            utxo_history_tip: true,
            outpoint: OutPoint {
                txid: txid2,
                vout: 1,
            },
            ancestor_tx_id: Some(ancestor_id),
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };

        // Ancestor in same tx as ancestor 1 (should be ignored)
        let ancestor2 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: Some(ancestor_batch_id),
            utxo_history_tip: false,
            outpoint: OutPoint {
                txid: ancestor_id,
                vout: 1,
            },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };

        let res = extract_cpfp_utxos(
            vec![candidate1, candidate2, candidate3, ancestor1, ancestor2]
                .into_iter()
                .collect(),
        );
        let utxos = res.get(&keychain_id).unwrap();
        assert_eq!(utxos.len(), 2);
        let accumulated = utxos.iter().find(|u| u.additional_vbytes() == 42).unwrap();
        assert_eq!(accumulated.included_fees(), Satoshis::from(42));
        let accumulated = utxos.iter().find(|u| u.additional_vbytes() == 84).unwrap();
        assert_eq!(accumulated.included_fees(), Satoshis::from(84));
    }

    #[test]
    fn accumulates_long_ancestor_chain() {
        let keychain_id = KeychainId::new();
        let txid = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let ancestor_id1 = "5010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let ancestor_id2 = "5011e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();

        let candidate1 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: None,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 0 },
            ancestor_tx_id: Some(ancestor_id1),
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let ancestor1 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: Some(BatchId::new()),
            utxo_history_tip: false,
            outpoint: OutPoint {
                txid: ancestor_id1,
                vout: 0,
            },
            ancestor_tx_id: Some(ancestor_id2),
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let ancestor2 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: Some(BatchId::new()),
            utxo_history_tip: false,
            outpoint: OutPoint {
                txid: ancestor_id2,
                vout: 0,
            },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };

        let res = extract_cpfp_utxos(vec![candidate1, ancestor1, ancestor2].into_iter().collect());
        let utxos = res.get(&keychain_id).unwrap();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].additional_vbytes(), 126);
        assert_eq!(utxos[0].included_fees(), Satoshis::from(126));
    }

    #[test]
    fn candidate_with_multiple_ancestors() {
        let keychain_id = KeychainId::new();
        let txid = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let ancestor_id1 = "5010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let ancestor_id2 = "5011e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();

        let candidate1 = CpfpCandidate {
            keychain_id,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 0 },
            ancestor_tx_id: Some(ancestor_id1),
            origin_tx_batch_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let candidate2 = CpfpCandidate {
            keychain_id,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 0 },
            ancestor_tx_id: Some(ancestor_id2),
            origin_tx_batch_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let ancestor1 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: Some(BatchId::new()),
            utxo_history_tip: false,
            outpoint: OutPoint {
                txid: ancestor_id1,
                vout: 0,
            },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };
        let ancestor2 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: Some(BatchId::new()),
            utxo_history_tip: false,
            outpoint: OutPoint {
                txid: ancestor_id2,
                vout: 0,
            },
            ancestor_tx_id: None,
            origin_tx_vbytes: 42,
            origin_tx_fee: Satoshis::from(42),
        };

        let res = extract_cpfp_utxos(
            vec![candidate1, candidate2, ancestor1, ancestor2]
                .into_iter()
                .collect(),
        );
        let utxos = res.get(&keychain_id).unwrap();
        dbg!(utxos);
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].additional_vbytes(), 126);
        assert_eq!(utxos[0].included_fees(), Satoshis::from(126));
    }

    #[test]
    fn candidate_with_cpfp_history() {
        let keychain_id = KeychainId::new();
        let txid = "4010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();
        let ancestor_id1 = "5010e27ff7dc6d9c66a5657e6b3d94b4c4e394d968398d16fefe4637463d194d"
            .parse()
            .unwrap();

        let candidate1 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: None,
            utxo_history_tip: true,
            outpoint: OutPoint { txid, vout: 0 },
            ancestor_tx_id: Some(ancestor_id1),
            origin_tx_vbytes: 100,
            origin_tx_fee: Satoshis::from(1000),
        };
        let ancestor1 = CpfpCandidate {
            keychain_id,
            origin_tx_batch_id: Some(BatchId::new()),
            utxo_history_tip: false,
            outpoint: OutPoint {
                txid: ancestor_id1,
                vout: 0,
            },
            ancestor_tx_id: None,
            origin_tx_vbytes: 100,
            origin_tx_fee: Satoshis::from(100),
        };

        let res = extract_cpfp_utxos(vec![candidate1, ancestor1].into_iter().collect());
        let utxos = res.get(&keychain_id).unwrap();
        assert_eq!(utxos.len(), 1);
        assert_eq!(utxos[0].additional_vbytes(), 200);
        assert_eq!(utxos[0].included_fees(), Satoshis::from(1100));
        let missing_fees = utxos[0].missing_fees(&FeeRate::from_sat_per_vb(100.0));
        assert_eq!(missing_fees.len(), 2);
        let (_, missing_fee) = missing_fees
            .get(&txid)
            .expect("missing missing fee for txid");
        assert_eq!(*missing_fee, Satoshis::from(9450));
        let (_, missing_fee) = missing_fees
            .get(&ancestor_id1)
            .expect("missing missing fee for txid");
        assert_eq!(*missing_fee, Satoshis::from(9450));
    }
}
