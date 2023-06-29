use bdk::bitcoin::psbt;

use std::collections::HashSet;

use crate::xpub::XPub;

pub fn validate_psbt(psbt: &psbt::PartiallySignedTransaction, xpub: XPub) -> bool {
    let set: HashSet<_> = psbt
        .inputs
        .iter()
        .flat_map(|inp| &inp.bip32_derivation)
        .filter_map(|(pk, (fingerprint, _))| {
            (fingerprint == &xpub.inner().parent_fingerprint).then_some(pk)
        })
        .collect();

    psbt.inputs
        .iter()
        .flat_map(|inp| &inp.partial_sigs)
        .any(|(pk, _)| set.contains(&pk.inner))
}
