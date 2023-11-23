use bdk::{
    database::BatchDatabase,
    wallet::{tx_builder::TxOrdering, AddressIndex, AddressInfo},
    FeeRate, Wallet,
};
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
};

use super::{keychain::*, Wallet as WalletEntity};
use crate::{
    bdk::error::BdkError,
    primitives::{bitcoin::*, *},
    utxo::CpfpUtxo,
};

pub const DEFAULT_SIGHASH_TYPE: bdk::bitcoin::EcdsaSighashType =
    bdk::bitcoin::EcdsaSighashType::All;
const HEADER_VBYTES: usize = 53;

pub struct WalletTotals {
    pub wallet_id: WalletId,
    pub change_keychain_id: KeychainId,
    pub keychains_with_inputs: Vec<KeychainId>,
    pub input_satoshis: Satoshis,
    pub output_satoshis: Satoshis,
    pub total_fee_satoshis: Satoshis,
    pub cpfp_fee_satoshis: Satoshis,
    pub cpfp_allocations: HashMap<Txid, (Option<BatchId>, Satoshis)>,
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

pub struct PsbtBuilder<T> {
    consolidate_deprecated_keychains: Option<bool>,
    fee_rate: Option<FeeRate>,
    reserved_utxos: Option<HashMap<KeychainId, Vec<OutPoint>>>,
    cpfp_utxos: Option<HashMap<KeychainId, Vec<CpfpUtxo>>>,
    missing_cpfp_fees: HashMap<Txid, (Option<BatchId>, Satoshis)>,
    current_wallet: Option<WalletId>,
    current_payouts: Vec<TxPayout>,
    current_wallet_psbts: Vec<(KeychainId, psbt::PartiallySignedTransaction)>,
    current_wallet_cpfp_allocations: HashMap<Txid, (Option<BatchId>, Satoshis)>,
    result: FinishedPsbtBuild,
    input_weights: HashMap<OutPoint, usize>,
    all_included_utxos: HashSet<OutPoint>,
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

        ret
    }
}

impl Default for PsbtBuilder<InitialPsbtBuilderState> {
    fn default() -> Self {
        Self::new()
    }
}

impl PsbtBuilder<InitialPsbtBuilderState> {
    pub async fn construct_psbt(
        pool: &sqlx::PgPool,
        consolidate_deprecated_keychains: bool,
        fee_rate: FeeRate,
        reserved_utxos: HashMap<KeychainId, Vec<bitcoin::OutPoint>>,
        cpfp_utxos: HashMap<KeychainId, Vec<CpfpUtxo>>,
        unbatched_payouts: HashMap<WalletId, Vec<TxPayout>>,
        mut wallets: HashMap<WalletId, WalletEntity>,
    ) -> Result<FinishedPsbtBuild, BdkError> {
        let mut outer_builder = PsbtBuilder::new()
            .consolidate_deprecated_keychains(consolidate_deprecated_keychains)
            .fee_rate(fee_rate)
            .reserved_utxos(reserved_utxos)
            .cpfp_utxos(cpfp_utxos)
            .accept_wallets();

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

    pub fn new() -> Self {
        Self {
            consolidate_deprecated_keychains: None,
            fee_rate: None,
            reserved_utxos: None,
            missing_cpfp_fees: HashMap::new(),
            cpfp_utxos: None,
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
            },
            _phantom: PhantomData,
        }
    }

    pub fn consolidate_deprecated_keychains(
        mut self,
        consolidate_deprecated_keychains: bool,
    ) -> Self {
        self.consolidate_deprecated_keychains = Some(consolidate_deprecated_keychains);
        self
    }

    pub fn reserved_utxos(mut self, reserved_utxos: HashMap<KeychainId, Vec<OutPoint>>) -> Self {
        self.reserved_utxos = Some(reserved_utxos);
        self
    }

    pub fn cpfp_utxos(mut self, cpfp_utxos: HashMap<KeychainId, Vec<CpfpUtxo>>) -> Self {
        self.cpfp_utxos = Some(cpfp_utxos);
        self
    }

    pub fn fee_rate(mut self, fee_rate: FeeRate) -> Self {
        self.fee_rate = Some(fee_rate);
        self
    }

    pub fn accept_wallets(mut self) -> PsbtBuilder<AcceptingWalletState> {
        assert!(self.fee_rate.is_some());
        let fee_rate = self.fee_rate.as_ref().unwrap();
        if let Some(utxos) = self.cpfp_utxos.as_ref() {
            for utxo in utxos.iter().flat_map(|(_, v)| v.iter()) {
                self.missing_cpfp_fees.extend(utxo.missing_fees(fee_rate));
            }
        };

        PsbtBuilder::<AcceptingWalletState> {
            consolidate_deprecated_keychains: self.consolidate_deprecated_keychains,
            fee_rate: self.fee_rate,
            reserved_utxos: self.reserved_utxos,
            missing_cpfp_fees: self.missing_cpfp_fees,
            cpfp_utxos: self.cpfp_utxos,
            current_wallet: None,
            current_payouts: vec![],
            current_wallet_psbts: self.current_wallet_psbts,
            current_wallet_cpfp_allocations: self.current_wallet_cpfp_allocations,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result: self.result,
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
            consolidate_deprecated_keychains: self.consolidate_deprecated_keychains,
            fee_rate: self.fee_rate,
            reserved_utxos: self.reserved_utxos,
            missing_cpfp_fees: self.missing_cpfp_fees,
            cpfp_utxos: self.cpfp_utxos,
            current_wallet: Some(wallet_id),
            current_payouts: payouts,
            current_wallet_psbts: self.current_wallet_psbts,
            current_wallet_cpfp_allocations: self.current_wallet_cpfp_allocations,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result: self.result,
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
        if !self.consolidate_deprecated_keychains.unwrap_or(false) {
            return Ok(self);
        }

        let keychain_satisfaction_weight = wallet
            .get_descriptor_for_keychain(KeychainKind::External)
            .max_satisfaction_weight()
            .expect("Unsupported descriptor");

        let drain_address = wallet.get_internal_address(AddressIndex::LastUnused)?;

        let mut builder = wallet.build_tx();
        if let Some(reserved_utxos) = self
            .reserved_utxos
            .as_ref()
            .and_then(|m| m.get(&keychain_id))
        {
            for out in reserved_utxos {
                builder.add_unspendable(*out);
            }
        }
        builder
            .fee_rate(self.fee_rate.expect("fee rate must be set"))
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
            consolidate_deprecated_keychains: self.consolidate_deprecated_keychains,
            fee_rate: self.fee_rate,
            reserved_utxos: self.reserved_utxos,
            missing_cpfp_fees: self.missing_cpfp_fees,
            cpfp_utxos: self.cpfp_utxos,
            current_wallet: self.current_wallet,
            current_payouts: self.current_payouts,
            current_wallet_psbts: self.current_wallet_psbts,
            current_wallet_cpfp_allocations: self.current_wallet_cpfp_allocations,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result: self.result,
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
        let change_address = wallet.get_internal_address(AddressIndex::LastUnused)?;
        let keychain_satisfaction_weight = wallet
            .get_descriptor_for_keychain(KeychainKind::External)
            .max_satisfaction_weight()
            .expect("Unsupported descriptor");
        let mut max_payout = 0;
        let mut absolute_fee = 0;
        let mut inputs = Vec::new();
        while max_payout < self.current_payouts.len() {
            let (fee, ins, success) = self.try_build_current_wallet_psbt(
                current_keychain_id,
                &self.current_payouts[..=max_payout],
                wallet,
                &change_address,
                keychain_satisfaction_weight,
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
            builder.fee_absolute(absolute_fee + self.fee_rate.unwrap().fee_vb(HEADER_VBYTES));
        } else {
            builder.fee_absolute(absolute_fee + u64::from(self.result.fee_satoshis));
        }

        builder.drain_to(change_address.script_pubkey());
        builder.sighash(DEFAULT_SIGHASH_TYPE.into());
        builder.manually_selected_only();
        builder.add_utxos(&inputs)?;

        if let Some(cpfp) = self
            .cpfp_utxos
            .as_ref()
            .map(|m| m.get(&current_keychain_id))
            .flatten()
        {
            for utxo in cpfp {
                for k in utxo.attributions.keys() {
                    self.current_wallet_cpfp_allocations
                        .insert(*k, self.missing_cpfp_fees.remove(k).expect("cpfp fees"));
                }
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

        for (keychain_id, psbt) in self.current_wallet_psbts.drain(..) {
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

        if let Some(result_psbt) = self.result.psbt {
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
                let cpfp_fee_satoshis = cpfp_allocations.values().map(|(_, s)| s).sum::<Satoshis>();
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
            consolidate_deprecated_keychains: self.consolidate_deprecated_keychains,
            fee_rate: self.fee_rate,
            reserved_utxos: self.reserved_utxos,
            missing_cpfp_fees: self.missing_cpfp_fees,
            cpfp_utxos: self.cpfp_utxos,
            current_wallet: None,
            current_payouts: vec![],
            current_wallet_psbts: self.current_wallet_psbts,
            current_wallet_cpfp_allocations: self.current_wallet_cpfp_allocations,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result: self.result,
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
        keychain_satisfaction_weight: usize,
    ) -> Result<(u64, Vec<OutPoint>, bool), BdkError> {
        let mut builder = wallet.build_tx();
        builder.fee_rate(self.fee_rate.expect("fee rate must be set"));
        builder.drain_to(change_address.script_pubkey());

        for (_, destination, satoshis) in payouts.iter() {
            builder.add_recipient(destination.script_pubkey(), u64::from(*satoshis));
        }

        if let Some(reserved_utxos) = self
            .reserved_utxos
            .as_ref()
            .and_then(|m| m.get(&keychain_id))
        {
            for out in reserved_utxos {
                builder.add_unspendable(*out);
            }
        }

        let mut cpfp_fees = 0;
        if let Some(cpfp) = self
            .cpfp_utxos
            .as_ref()
            .map(|m| m.get(&keychain_id))
            .flatten()
        {
            for utxo in cpfp {
                for k in utxo.attributions.keys() {
                    cpfp_fees +=
                        u64::from(self.missing_cpfp_fees.get(k).expect("missing cpfp fees").1);
                }
                builder.add_utxo(utxo.outpoint)?;
            }
            builder.add_recipient(change_address.script_pubkey(), cpfp_fees);
        }

        let mut foreign_utxos = HashSet::new();
        for (_, psbt) in self.current_wallet_psbts.iter() {
            for (input, psbt_input) in psbt.unsigned_tx.input.iter().zip(psbt.inputs.iter()) {
                foreign_utxos.insert(input.previous_output);
                builder.add_foreign_utxo(
                    input.previous_output,
                    psbt_input.clone(),
                    *self
                        .input_weights
                        .get(&input.previous_output)
                        .expect("weight should always be present"),
                )?;
            }
        }

        match builder.finish() {
            Ok((psbt, details)) => {
                let n_change_outputs = psbt
                    .unsigned_tx
                    .output
                    .iter()
                    .filter(|out| out.script_pubkey == change_address.script_pubkey())
                    .count();
                let subtract_fee = if n_change_outputs > 1 {
                    self.fee_rate
                        .expect("fee rate must be set")
                        .fee_wu(keychain_satisfaction_weight)
                        + self.fee_rate.unwrap().fee_vb(HEADER_VBYTES)
                } else {
                    self.fee_rate
                        .expect("fee rate must be set")
                        .fee_vb(HEADER_VBYTES)
                };
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
            Err(bdk::Error::InsufficientFunds { .. }) => Ok((0, Vec::new(), false)),
            Err(e) => Err(e.into()),
        }
    }
}
