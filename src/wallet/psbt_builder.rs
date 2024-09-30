use bdk::{
    bitcoin::hashes::Hash,
    database::BatchDatabase,
    wallet::{tx_builder::TxOrdering, AddressIndex, AddressInfo},
    FeeRate, Wallet,
};
use derive_builder::Builder;
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
};
use tracing::instrument;

use super::{keychain::*, Wallet as WalletEntity};
use crate::{
    bdk::error::BdkError,
    primitives::{bitcoin::*, *},
    utxo::CpfpUtxo,
};

pub const DEFAULT_SIGHASH_TYPE: bdk::bitcoin::sighash::EcdsaSighashType =
    bdk::bitcoin::sighash::EcdsaSighashType::All;
const HEADER_VBYTES: usize = 53;
const MAX_JITTER_PERCENT: u64 = 10; //10%

pub struct WalletTotals {
    pub wallet_id: WalletId,
    pub change_keychain_id: KeychainId,
    pub keychains_with_inputs: Vec<KeychainId>,
    pub input_satoshis: Satoshis,
    pub output_satoshis: Satoshis,
    pub total_fee_satoshis: Satoshis,
    pub cpfp_fee_satoshis: Satoshis,
    pub cpfp_allocations: HashMap<OutPoint, HashMap<Txid, (Option<BatchId>, Satoshis)>>,
    pub change_satoshis: Satoshis,
    pub change_address: AddressInfo,
    pub change_outpoint: Option<OutPoint>,
}

pub struct FinishedPsbtBuild {
    pub included_payouts: HashMap<WalletId, Vec<(TxPayout, u32)>>,
    pub included_utxos: HashMap<WalletId, HashMap<KeychainId, Vec<bitcoin::OutPoint>>>,
    pub included_wallet_keychains: HashMap<KeychainId, WalletId>,
    pub wallet_totals: HashMap<WalletId, WalletTotals>,
    pub fee_satoshis: Satoshis,
    pub tx_id: Option<bitcoin::Txid>,
    pub psbt: Option<psbt::PartiallySignedTransaction>,
    pub provisional_proposal: Option<payjoin::receive::v2::ProvisionalProposal>,
}

impl FinishedPsbtBuild {
    pub fn proportional_fee(
        &self,
        wallet_id: &WalletId,
        payout_amount: Satoshis,
    ) -> Option<Satoshis> {
        self.wallet_totals.get(wallet_id).map(|total| {
            let proportion = payout_amount.into_inner() / total.output_satoshis.into_inner();
            let proportional_fee = total.total_fee_satoshis.into_inner() * proportion;
            Satoshis::from(
                proportional_fee
                    .round_dp_with_strategy(0, rust_decimal::RoundingStrategy::AwayFromZero),
            )
        })
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct PsbtBuilderConfig {
    consolidate_deprecated_keychains: bool,
    fee_rate: FeeRate,
    #[builder(default)]
    reserved_utxos: HashMap<KeychainId, Vec<OutPoint>>,
    #[builder(default)]
    cpfp_utxos: HashMap<KeychainId, Vec<CpfpUtxo>>,
    #[builder(default)]
    for_estimation: bool,
    #[builder(default)]
    force_min_change_output: Option<Satoshis>,
    #[builder(default)]
    wants_outputs: Option<payjoin::receive::v2::WantsOutputs>,
}

impl PsbtBuilderConfig {
    pub fn builder() -> PsbtBuilderConfigBuilder {
        PsbtBuilderConfigBuilder::default()
    }

    pub fn collect_missing_cpfp_fees(&mut self) -> HashMap<Txid, (Option<BatchId>, Satoshis)> {
        let mut missing_cpfp_fees = HashMap::new();
        let mut non_cpfp_utxos = HashSet::new();
        for (keychain, utxos) in self.cpfp_utxos.iter_mut() {
            non_cpfp_utxos.clear();
            for utxo in utxos.iter() {
                let missing_fees = utxo.missing_fees(&self.fee_rate);
                // If we don't actually need additional fees to bump we will
                // let BDK choose the UTXO it if it wants
                // => remove from reserved_utxos
                if missing_fees.is_empty() {
                    non_cpfp_utxos.insert(utxo.outpoint);
                    if let Some(reserved) = self.reserved_utxos.get_mut(keychain) {
                        reserved.retain(|out| out != &utxo.outpoint);
                    }
                } else {
                    missing_cpfp_fees.extend(missing_fees);
                }
            }
            utxos.retain(|utxo| !non_cpfp_utxos.contains(&utxo.outpoint));
        }
        missing_cpfp_fees
    }

    fn force_min_change_output_with_jitter(&self) -> Option<Satoshis> {
        use rand::Rng;
        self.force_min_change_output.map(|sats| {
            let max_jitter = u64::from(sats) * MAX_JITTER_PERCENT / 100;
            let jitter: u64 = rand::thread_rng().gen_range(0..=max_jitter);
            sats + Satoshis::from(jitter)
        })
    }
}

pub struct PsbtBuilder<T> {
    cfg: PsbtBuilderConfig,
    missing_cpfp_fees: HashMap<Txid, (Option<BatchId>, Satoshis)>,
    current_wallet: Option<WalletId>,
    current_payouts: Vec<TxPayout>,
    current_wallet_psbts: Vec<(KeychainId, psbt::PartiallySignedTransaction)>,
    current_wallet_cpfp_allocations: HashMap<OutPoint, HashMap<Txid, (Option<BatchId>, Satoshis)>>,
    result: FinishedPsbtBuild,
    input_weights: HashMap<OutPoint, usize>,
    all_included_utxos: HashSet<OutPoint>,
    provisional_proposal: Option<payjoin::receive::v2::ProvisionalProposal>,
    _phantom: PhantomData<T>,
}

pub struct InitialPsbtBuilderState;
pub struct AcceptingWalletState;
pub struct AcceptingDeprecatedKeychainState;
pub struct AcceptingCurrentKeychainState;

impl<T> PsbtBuilder<T> {
    fn finish_inner(self) -> FinishedPsbtBuild {
        let mut ret = self.result;
        let mut outpoints = HashSet::new();
        if let (Some(tx_id), Some(psbt)) = (ret.tx_id.as_mut(), ret.psbt.as_ref()) {
            let outputs = &psbt.unsigned_tx.output;

            // Identify change outputs
            for (_, total) in ret.wallet_totals.iter_mut() {
                if total.change_satoshis == Satoshis::ZERO {
                    continue;
                }
                let (vout, _) = outputs
                    .iter()
                    .enumerate()
                    .find(|(_, out)| {
                        out.script_pubkey == total.change_address.script_pubkey()
                            && Satoshis::from(out.value) == total.change_satoshis
                    })
                    .expect("change output disappeared");
                total.change_outpoint = Some(OutPoint {
                    txid: *tx_id,
                    vout: vout as u32,
                });
                outpoints.insert(vout);
            }

            // Identify vout for payouts
            for payouts in ret.included_payouts.values_mut() {
                for ((_, addr, sats), vout) in payouts.iter_mut() {
                    let (found, _) = outputs
                        .iter()
                        .enumerate()
                        .find(|(vout, out)| {
                            if outpoints.contains(vout) {
                                return false;
                            }
                            out.script_pubkey == addr.script_pubkey()
                                && Satoshis::from(out.value) == *sats
                        })
                        .expect("payout output disappeared");
                    *vout = found as u32;
                    outpoints.insert(found);
                }
            }
        }

        // Identify signing keychains
        for (wallet_id, keychain_utxos) in ret.included_utxos.iter() {
            let sum = ret
                .wallet_totals
                .get_mut(wallet_id)
                .expect("wallet not included in totals");
            sum.keychains_with_inputs
                .extend(keychain_utxos.keys().copied());
        }
        ret.provisional_proposal = self.provisional_proposal;
        ret
    }
}

impl PsbtBuilder<InitialPsbtBuilderState> {
    #[instrument(name = "psbt_builder.construct_psbt", skip_all)]
    pub async fn construct_psbt(
        pool: &sqlx::PgPool,
        cfg: PsbtBuilderConfig,
        unbatched_payouts: HashMap<WalletId, Vec<TxPayout>>,
        mut wallets: HashMap<WalletId, WalletEntity>,
    ) -> Result<FinishedPsbtBuild, BdkError> {
        let mut outer_builder = PsbtBuilder::new(cfg);

        for (wallet_id, payouts) in unbatched_payouts {
            let wallet = wallets.remove(&wallet_id).expect("Wallet not found");

            let mut builder = outer_builder.wallet_payouts(wallet.id, payouts);
            for keychain in wallet.deprecated_keychain_wallets(pool.clone()) {
                builder = keychain.dispatch_bdk_wallet(builder).await?;
            }
            outer_builder = wallet
                .current_keychain_wallet(pool)
                .dispatch_bdk_wallet(builder.accept_current_keychain())
                .await?
                .next_wallet();
        }
        Ok(outer_builder.finish())
    }

    pub fn new(mut cfg: PsbtBuilderConfig) -> PsbtBuilder<AcceptingWalletState> {
        let missing_cpfp_fees = cfg.collect_missing_cpfp_fees();
        PsbtBuilder::<AcceptingWalletState> {
            cfg,
            missing_cpfp_fees,
            current_wallet: None,
            current_payouts: vec![],
            current_wallet_psbts: vec![],
            current_wallet_cpfp_allocations: HashMap::new(),
            all_included_utxos: HashSet::new(),
            input_weights: HashMap::new(),
            result: FinishedPsbtBuild {
                included_payouts: HashMap::new(),
                included_utxos: HashMap::new(),
                included_wallet_keychains: HashMap::new(),
                wallet_totals: HashMap::new(),
                fee_satoshis: Satoshis::from(0),
                tx_id: None,
                psbt: None,
                provisional_proposal: None,
            },
            provisional_proposal: None,
            _phantom: PhantomData,
        }
    }
}

impl PsbtBuilder<AcceptingWalletState> {
    pub fn wallet_payouts(
        self,
        wallet_id: WalletId,
        payouts: Vec<TxPayout>,
    ) -> PsbtBuilder<AcceptingDeprecatedKeychainState> {
        assert!(self.current_wallet_psbts.is_empty());
        assert!(self.current_wallet_cpfp_allocations.is_empty());
        PsbtBuilder::<AcceptingDeprecatedKeychainState> {
            cfg: self.cfg,
            missing_cpfp_fees: self.missing_cpfp_fees,
            current_wallet: Some(wallet_id),
            current_payouts: payouts,
            current_wallet_psbts: self.current_wallet_psbts,
            current_wallet_cpfp_allocations: self.current_wallet_cpfp_allocations,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result: self.result,
            provisional_proposal: self.provisional_proposal,
            _phantom: PhantomData,
        }
    }

    pub fn finish(self) -> FinishedPsbtBuild {
        self.finish_inner()
    }
}

impl BdkWalletVisitor for PsbtBuilder<AcceptingDeprecatedKeychainState> {
    fn visit_bdk_wallet<D: BatchDatabase>(
        mut self,
        keychain_id: KeychainId,
        wallet: &Wallet<D>,
    ) -> Result<Self, BdkError> {
        if !self.cfg.consolidate_deprecated_keychains {
            return Ok(self);
        }

        let keychain_satisfaction_weight = wallet
            .get_descriptor_for_keychain(KeychainKind::External)
            .max_satisfaction_weight()
            .expect("Unsupported descriptor");

        let drain_address = if self.cfg.for_estimation {
            wallet.get_internal_address(AddressIndex::Peek(0))?
        } else {
            wallet.get_internal_address(AddressIndex::LastUnused)?
        };

        let mut builder = wallet.build_tx();
        if let Some(reserved_utxos) = self.cfg.reserved_utxos.get(&keychain_id) {
            for out in reserved_utxos {
                builder.add_unspendable(*out);
            }
        }
        builder
            .fee_rate(self.cfg.fee_rate)
            .sighash(DEFAULT_SIGHASH_TYPE.into())
            .drain_wallet()
            .drain_to(drain_address.script_pubkey());
        match builder.finish() {
            Ok((psbt, _details)) => {
                for input in psbt.unsigned_tx.input.iter() {
                    self.input_weights
                        .insert(input.previous_output, keychain_satisfaction_weight);
                }
                self.current_wallet_psbts.push((keychain_id, psbt));
                Ok(self)
            }
            Err(e) => {
                dbg!(e);
                unimplemented!()
            }
        }
    }
}

impl PsbtBuilder<AcceptingDeprecatedKeychainState> {
    pub fn accept_current_keychain(self) -> PsbtBuilder<AcceptingCurrentKeychainState> {
        PsbtBuilder::<AcceptingCurrentKeychainState> {
            cfg: self.cfg,
            missing_cpfp_fees: self.missing_cpfp_fees,
            current_wallet: self.current_wallet,
            current_payouts: self.current_payouts,
            current_wallet_psbts: self.current_wallet_psbts,
            current_wallet_cpfp_allocations: self.current_wallet_cpfp_allocations,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result: self.result,
            provisional_proposal: self.provisional_proposal,
            _phantom: PhantomData,
        }
    }
}

impl BdkWalletVisitor for PsbtBuilder<AcceptingCurrentKeychainState> {
    fn visit_bdk_wallet<D: BatchDatabase>(
        mut self,
        current_keychain_id: KeychainId,
        wallet: &Wallet<D>,
    ) -> Result<Self, BdkError> {
        let change_address = if self.cfg.for_estimation {
            wallet.get_internal_address(AddressIndex::Peek(0))?
        } else {
            wallet.get_internal_address(AddressIndex::LastUnused)?
        };
        let keychain_satisfaction_weight = wallet
            .get_descriptor_for_keychain(KeychainKind::External)
            .max_satisfaction_weight()
            .expect("Unsupported descriptor");
        let mut max_payout = 0;
        let mut absolute_fee = 0;
        let mut inputs = Vec::new();
        if let Some(wants_outputs) = &self.cfg.wants_outputs {
            for pj_txin in wants_outputs.original_psbt().unsigned_tx.input.iter() {
                use std::str::FromStr;
                // FIXME weight should be paid for by payjoin sender
                let bdk_outpoint = OutPoint {
                    txid: bdk::bitcoin::Txid::from_str(&pj_txin.previous_output.txid.to_string())
                        .unwrap(),
                    vout: pj_txin.previous_output.vout,
                };
                // input weights must be added for try_build
                self.input_weights.insert(bdk_outpoint, 0);
            }
        }
        while max_payout < self.current_payouts.len() {
            let (fee, ins, success) = self.try_build_current_wallet_psbt(
                current_keychain_id,
                &self.current_payouts[..=max_payout],
                wallet,
                &change_address,
            )?;
            if !success {
                break;
            }
            absolute_fee = fee;
            max_payout += 1;
            inputs = ins;
        }
        if max_payout == 0 {
            return Ok(self);
        }

        let mut builder = wallet.build_tx();
        if self.result.fee_satoshis == Satoshis::ZERO {
            builder.fee_absolute(absolute_fee + self.cfg.fee_rate.fee_vb(HEADER_VBYTES));
        } else {
            builder.fee_absolute(absolute_fee + u64::from(self.result.fee_satoshis));
        }

        builder.drain_to(change_address.script_pubkey());
        builder.sighash(DEFAULT_SIGHASH_TYPE.into());
        builder.manually_selected_only();
        builder.add_utxos(&inputs)?;

        if let Some(cpfp) = self.cfg.cpfp_utxos.get(&current_keychain_id) {
            for utxo in cpfp {
                let mut tx_allocations = HashMap::new();
                for k in utxo.attributions.keys() {
                    if let Some(fee) = self.missing_cpfp_fees.remove(k) {
                        tx_allocations.insert(*k, fee);
                    }
                }
                self.current_wallet_cpfp_allocations
                    .insert(utxo.outpoint, tx_allocations);
            }
        }

        let mut total_output_satoshis = Satoshis::from(0);
        for (payout_id, destination, satoshis) in self.current_payouts.drain(..max_payout) {
            total_output_satoshis += satoshis;
            builder.add_recipient(destination.script_pubkey(), u64::from(satoshis));
            self.result
                .included_payouts
                .entry(self.current_wallet.expect("current wallet must be set"))
                .or_default()
                .push(((payout_id, destination, satoshis), 0));
        }

        // add foreign payjoin utxos
        // *try* Handle payjoin to see what happens. visit_bdk_wallet will actually use the state machine
        let payjoin_original_psbt = if let Some(ref wants_outputs) = self.cfg.wants_outputs {
            use std::str::FromStr;
            let mut payjoin_original_psbt =
                psbt::Psbt::from_str(&wants_outputs.original_psbt().to_string())
                    .expect("failed to parse payjoin original psbt");
            let current_wallet_owned_vouts = wants_outputs.owned_vouts();
            for i in (0..payjoin_original_psbt.unsigned_tx.output.len()).rev() {
                if !current_wallet_owned_vouts.contains(&i) {
                    payjoin_original_psbt.outputs.remove(i);
                    payjoin_original_psbt.unsigned_tx.output.remove(i);
                }
            }
            // for include each remaining payjoin output
            for output in payjoin_original_psbt.unsigned_tx.output.iter() {
                builder.add_recipient(output.script_pubkey.clone(), output.value);
            }
            Some((current_keychain_id, payjoin_original_psbt))
        } else {
            None
        };
        dbg!(
            "is_payjoin_original_psbt: {:?}",
            &payjoin_original_psbt.is_some()
        );
        if let Some((keychain_id, payjoin_original_psbt)) = payjoin_original_psbt {
            self.current_wallet_psbts
                .push((keychain_id, payjoin_original_psbt));
        }
        dbg!(
            "current_wallet_psbts.len(): {:?}",
            &self.current_wallet_psbts.len()
        );
        for (keychain_id, psbt) in self.current_wallet_psbts.drain(..) {
            dbg!("keychainid drain: {:?}", &keychain_id);
            for (input, psbt_input) in psbt.unsigned_tx.input.into_iter().zip(psbt.inputs) {
                builder.add_foreign_utxo(
                    input.previous_output,
                    psbt_input,
                    *self
                        .input_weights
                        .get(&input.previous_output)
                        .expect("weight should always be present"),
                )?;
                self.result
                    .included_utxos
                    .entry(self.current_wallet.unwrap())
                    .or_default()
                    .entry(keychain_id)
                    .or_default()
                    .push(input.previous_output);
                self.result.included_wallet_keychains.insert(
                    keychain_id,
                    self.current_wallet.expect("current wallet shouyld be set"),
                );
                self.all_included_utxos.insert(input.previous_output);
            }
        }
        dbg!("Does result.psbt exist? {}", self.result.psbt.is_some());
        if let Some(result_psbt) = self.result.psbt {
            dbg!("result_psbt exists");
            for (input, psbt_input) in result_psbt
                .unsigned_tx
                .input
                .into_iter()
                .zip(result_psbt.inputs)
            {
                builder.add_foreign_utxo(
                    input.previous_output,
                    psbt_input,
                    *self
                        .input_weights
                        .get(&input.previous_output)
                        .expect("weight should always be present"),
                )?;
            }

            for out in result_psbt.unsigned_tx.output {
                builder.add_recipient(out.script_pubkey, out.value);
            }
        }

        builder.ordering(TxOrdering::Bip69Lexicographic);
        match builder.finish() {
            Ok((psbt, details)) => {
                if let Some(wants_outputs) = &self.cfg.wants_outputs {
                    use std::str::FromStr;
                    // convert psbt unsigned_tx.output to payjoin::bitcoin::TxOut
                    let replacement_outputs: Vec<payjoin::bitcoin::TxOut> = psbt
                        .unsigned_tx
                        .output
                        .clone()
                        .into_iter()
                        .map(|out| payjoin::bitcoin::TxOut {
                            value: payjoin::bitcoin::Amount::from_sat(out.value.into()),
                            script_pubkey: payjoin::bitcoin::ScriptBuf::from_bytes(
                                out.script_pubkey.to_bytes(),
                            ),
                        })
                        .collect();
                    let payjoin_drain_script = payjoin::bitcoin::ScriptBuf::from_bytes(
                        change_address.script_pubkey().to_bytes(),
                    );
                    // TODO provide a receiver only output list, ignore sender change
                    let wants_inputs = wants_outputs
                        .clone()
                        .replace_receiver_outputs(replacement_outputs, &payjoin_drain_script)
                        .unwrap()
                        .commit_outputs();

                    let inputs: Vec<_> = psbt
                        .unsigned_tx
                        .input
                        .clone()
                        .into_iter()
                        .zip(psbt.inputs.clone())
                        .map(|(txin, psbt_input)| {
                            (
                                payjoin::bitcoin::OutPoint::new(
                                    payjoin::bitcoin::Txid::from_str(
                                        &txin.previous_output.txid.to_string(),
                                    )
                                    .unwrap(),
                                    txin.previous_output.vout,
                                ),
                                payjoin::bitcoin::TxOut {
                                    value: payjoin::bitcoin::Amount::from_sat(
                                        psbt_input.witness_utxo.clone().unwrap().value.into(),
                                    ),
                                    script_pubkey: payjoin::bitcoin::ScriptBuf::from_bytes(
                                        psbt_input.witness_utxo.unwrap().script_pubkey.to_bytes(),
                                    ),
                                },
                            )
                        })
                        .collect();
                    self.provisional_proposal = Some(
                        wants_inputs
                            .contribute_witness_inputs(inputs)
                            .unwrap()
                            .commit_inputs(),
                    );
                } // FIXME I think the fee is definitely wrong since proposal.apply_fee has not been called
                let fee_satoshis = Satoshis::from(details.fee.expect("fee must be present"));
                let current_wallet_fee = fee_satoshis - self.result.fee_satoshis;
                let wallet_id = self.current_wallet.expect("current wallet must be set");
                let change_satoshis = Satoshis::from(
                    psbt.unsigned_tx
                        .output
                        .iter()
                        .find(|out| out.script_pubkey == change_address.script_pubkey())
                        .map(|out| out.value)
                        .unwrap_or(0),
                );
                let mut cpfp_allocations = HashMap::new();
                std::mem::swap(
                    &mut cpfp_allocations,
                    &mut self.current_wallet_cpfp_allocations,
                );
                let cpfp_fee_satoshis = cpfp_allocations
                    .values()
                    .flat_map(|tx_allocations| tx_allocations.values())
                    .map(|(_, fee)| fee)
                    .sum::<Satoshis>();
                self.result.wallet_totals.insert(
                    wallet_id,
                    WalletTotals {
                        wallet_id,
                        keychains_with_inputs: Vec::new(),
                        input_satoshis: total_output_satoshis
                            + current_wallet_fee
                            + change_satoshis,
                        output_satoshis: total_output_satoshis,
                        total_fee_satoshis: current_wallet_fee,
                        cpfp_fee_satoshis,
                        cpfp_allocations,
                        change_satoshis,
                        change_address,
                        change_keychain_id: current_keychain_id,
                        change_outpoint: None,
                    },
                );
                self.result.fee_satoshis = fee_satoshis;

                for input in psbt.unsigned_tx.input.iter() {
                    self.input_weights
                        .insert(input.previous_output, keychain_satisfaction_weight);
                    if self.all_included_utxos.insert(input.previous_output) {
                        self.result
                            .included_utxos
                            .entry(wallet_id)
                            .or_default()
                            .entry(current_keychain_id)
                            .or_default()
                            .push(input.previous_output);
                        self.result.included_wallet_keychains.insert(
                            current_keychain_id,
                            self.current_wallet.expect("current wallet shouyld be set"),
                        );
                    }
                }
                self.result.psbt = Some(psbt);
                self.result.tx_id = Some(details.txid);
                Ok(self)
            }
            Err(e) => {
                dbg!(e);
                unimplemented!()
            }
        }
    }
}

impl PsbtBuilder<AcceptingCurrentKeychainState> {
    pub fn next_wallet(self) -> PsbtBuilder<AcceptingWalletState> {
        PsbtBuilder::<AcceptingWalletState> {
            cfg: self.cfg,
            missing_cpfp_fees: self.missing_cpfp_fees,
            current_wallet: None,
            current_payouts: vec![],
            current_wallet_psbts: self.current_wallet_psbts,
            current_wallet_cpfp_allocations: self.current_wallet_cpfp_allocations,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result: self.result,
            provisional_proposal: self.provisional_proposal,
            _phantom: PhantomData,
        }
    }

    pub fn finish(self) -> FinishedPsbtBuild {
        self.finish_inner()
    }

    fn try_build_current_wallet_psbt<D: BatchDatabase>(
        &self,
        keychain_id: KeychainId,
        payouts: &[TxPayout],
        wallet: &Wallet<D>,
        change_address: &AddressInfo,
    ) -> Result<(u64, Vec<OutPoint>, bool), BdkError> {
        let mut builder = wallet.build_tx();
        builder.fee_rate(self.cfg.fee_rate);
        builder.drain_to(change_address.script_pubkey());
        if let Some(sats_with_jitter) = self.cfg.force_min_change_output_with_jitter() {
            builder.add_recipient(change_address.script_pubkey(), u64::from(sats_with_jitter));
        }

        for (_, destination, satoshis) in payouts.iter() {
            builder.add_recipient(destination.script_pubkey(), u64::from(*satoshis));
        }

        if let Some(reserved_utxos) = self.cfg.reserved_utxos.get(&keychain_id) {
            for out in reserved_utxos {
                builder.add_unspendable(*out);
            }
        }

        let mut cpfp_fees = 0;
        if let Some(cpfp) = self.cfg.cpfp_utxos.get(&keychain_id) {
            for utxo in cpfp {
                for k in utxo.attributions.keys() {
                    if let Some((_, fee)) = self.missing_cpfp_fees.get(k) {
                        cpfp_fees += u64::from(*fee);
                    }
                }
                builder.add_utxo(utxo.outpoint)?;
            }

            if cpfp_fees > 0 {
                builder.allow_dust(true);
                builder.add_recipient(change_address.script_pubkey(), cpfp_fees);
            }
        }

        let mut foreign_utxos = HashSet::new();
        let mut input_weights = self.input_weights.clone();
        // add foreign payjoin utxos
        // *try* Handle payjoin to see what happens. visit_bdk_wallet will actually use the state machine
        let payjoin_original_psbt = if let Some(wants_outputs) = &self.cfg.wants_outputs {
            use std::str::FromStr;
            let mut payjoin_original_psbt =
                psbt::Psbt::from_str(&wants_outputs.original_psbt().to_string())
                    .expect("failed to parse payjoin original psbt");
            let current_wallet_owned_vouts = wants_outputs.owned_vouts();
            for i in (0..payjoin_original_psbt.unsigned_tx.output.len()).rev() {
                if !current_wallet_owned_vouts.contains(&i) {
                    payjoin_original_psbt.outputs.remove(i);
                    payjoin_original_psbt.unsigned_tx.output.remove(i);
                }
            }
            // for each remaining output, still pay that change
            for output in payjoin_original_psbt.unsigned_tx.output.iter() {
                builder.add_recipient(output.script_pubkey.clone(), output.value);
            }
            for input in payjoin_original_psbt.unsigned_tx.input.iter() {
                // FIXME weight should be paid for by payjoin sender
                let bdk_outpoint = OutPoint {
                    txid: bdk::bitcoin::Txid::from_str(&input.previous_output.txid.to_string())
                        .unwrap(),
                    vout: input.previous_output.vout,
                };
                // input weights must be added for try_build
                input_weights.insert(bdk_outpoint, 0);
            }

            // add inputs in following loop
            Some((keychain_id, payjoin_original_psbt))
        } else {
            None
        };

        let mut current_wallet_psbts = self.current_wallet_psbts.clone();
        if let Some((keychain_id, payjoin_original_psbt)) = payjoin_original_psbt {
            current_wallet_psbts.push((keychain_id, payjoin_original_psbt));
        }
        for (_, psbt) in current_wallet_psbts.iter() {
            for (input, psbt_input) in psbt.unsigned_tx.input.iter().zip(psbt.inputs.iter()) {
                foreign_utxos.insert(input.previous_output);
                builder.add_foreign_utxo(
                    input.previous_output,
                    psbt_input.clone(),
                    *input_weights
                        .get(&input.previous_output)
                        .expect("weight should always be present"),
                )?;
            }
        }

        match builder.finish() {
            Ok((psbt, details)) => {
                let script_pubkey = change_address.script_pubkey();
                let n_change_outputs = psbt
                    .unsigned_tx
                    .output
                    .iter()
                    .filter(|out| out.script_pubkey == script_pubkey)
                    .count();
                // Here we are subtracting HEADER_VBYTES because we only need them 1 time
                // and they will be added later if this is the first TX we are building
                // also removing redundant change_output weight
                let subtract_fee = self.cfg.fee_rate.fee_vb(HEADER_VBYTES)
                    + (1.max(n_change_outputs) - 1) as u64
                        * crate::fees::output_fee(&self.cfg.fee_rate, script_pubkey);
                let inputs = psbt
                    .unsigned_tx
                    .input
                    .into_iter()
                    .filter_map(|i| {
                        if foreign_utxos.contains(&i.previous_output) {
                            None
                        } else {
                            Some(i.previous_output)
                        }
                    })
                    .collect();
                Ok((
                    details.fee.expect("fee must be present") + cpfp_fees - subtract_fee,
                    inputs,
                    true,
                ))
            }
            // TODO different case for payjoin?
            Err(bdk::Error::InsufficientFunds { .. }) => Ok((0, Vec::new(), false)),
            Err(e) => Err(e.into()),
        }
    }
}
