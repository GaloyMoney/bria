use bdk::{
    database::BatchDatabase, wallet::tx_builder::TxOrdering, wallet::AddressIndex, FeeRate,
    KeychainKind, Wallet,
};
use bitcoin::{blockdata::transaction::OutPoint, util::psbt};
use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
};

use super::keychain::*;
use crate::{error::*, payout::Payout, primitives::*};

pub struct PsbtBuilder<T> {
    consolidate_deprecated_keychains: Option<bool>,
    fee_rate: Option<FeeRate>,
    current_wallet: Option<WalletId>,
    current_payouts: Vec<Payout>,
    current_wallet_psbts: Vec<(KeychainId, psbt::PartiallySignedTransaction)>,
    input_weights: HashMap<OutPoint, usize>,
    included_payouts: HashMap<WalletId, Vec<Payout>>,
    included_utxos: HashMap<KeychainId, Vec<OutPoint>>,
    all_included_utxos: HashSet<OutPoint>,
    result_psbt: Option<psbt::PartiallySignedTransaction>,
    _phantom: PhantomData<T>,
}

pub struct InitialPsbtBuilderState;
pub struct AcceptingWalletState;
pub struct AcceptingDeprecatedKeychainState;
pub struct AcceptingCurrentKeychainState;

impl PsbtBuilder<InitialPsbtBuilderState> {
    pub fn new() -> Self {
        Self {
            consolidate_deprecated_keychains: None,
            fee_rate: None,
            current_wallet: None,
            current_payouts: vec![],
            current_wallet_psbts: vec![],
            included_payouts: HashMap::new(),
            included_utxos: HashMap::new(),
            all_included_utxos: HashSet::new(),
            input_weights: HashMap::new(),
            result_psbt: None,
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

    pub fn fee_rate(mut self, fee_rate: FeeRate) -> Self {
        self.fee_rate = Some(fee_rate);
        self
    }

    pub fn accept_wallets(self) -> PsbtBuilder<AcceptingWalletState> {
        PsbtBuilder::<AcceptingWalletState> {
            consolidate_deprecated_keychains: self.consolidate_deprecated_keychains,
            fee_rate: self.fee_rate,
            current_wallet: None,
            current_payouts: vec![],
            current_wallet_psbts: self.current_wallet_psbts,
            included_payouts: self.included_payouts,
            included_utxos: self.included_utxos,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result_psbt: self.result_psbt,
            _phantom: PhantomData,
        }
    }
}

impl PsbtBuilder<AcceptingWalletState> {
    pub fn wallet_payouts(
        self,
        wallet_id: WalletId,
        payouts: Vec<Payout>,
    ) -> PsbtBuilder<AcceptingDeprecatedKeychainState> {
        assert!(self.current_wallet_psbts.is_empty());
        PsbtBuilder::<AcceptingDeprecatedKeychainState> {
            consolidate_deprecated_keychains: self.consolidate_deprecated_keychains,
            fee_rate: self.fee_rate,
            current_wallet: Some(wallet_id),
            current_payouts: payouts,
            current_wallet_psbts: self.current_wallet_psbts,
            included_payouts: self.included_payouts,
            included_utxos: self.included_utxos,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result_psbt: self.result_psbt,
            _phantom: PhantomData,
        }
    }
}

impl BdkWalletVisitor for PsbtBuilder<AcceptingDeprecatedKeychainState> {
    fn visit_bdk_wallet<D: BatchDatabase>(
        mut self,
        keychain_id: KeychainId,
        wallet: &Wallet<D>,
    ) -> Result<Self, BriaError> {
        if !self.consolidate_deprecated_keychains.unwrap_or(false) {
            return Ok(self);
        }

        let keychain_satisfaction_weight = wallet
            .get_descriptor_for_keychain(KeychainKind::External)
            .max_satisfaction_weight()
            .expect("Unsupported descriptor");

        let drain_address = wallet.get_internal_address(AddressIndex::LastUnused)?;

        let mut builder = wallet.build_tx();
        builder
            .fee_rate(self.fee_rate.expect("fee rate must be set"))
            .sighash(bitcoin::EcdsaSighashType::All.into())
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
            current_wallet: self.current_wallet,
            current_payouts: self.current_payouts,
            current_wallet_psbts: self.current_wallet_psbts,
            included_payouts: self.included_payouts,
            included_utxos: self.included_utxos,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result_psbt: self.result_psbt,
            _phantom: PhantomData,
        }
    }
}

impl BdkWalletVisitor for PsbtBuilder<AcceptingCurrentKeychainState> {
    fn visit_bdk_wallet<D: BatchDatabase>(
        mut self,
        keychain_id: KeychainId,
        wallet: &Wallet<D>,
    ) -> Result<Self, BriaError> {
        let keychain_satisfaction_weight = wallet
            .get_descriptor_for_keychain(KeychainKind::External)
            .max_satisfaction_weight()
            .expect("Unsupported descriptor");

        let mut max_payout = 0;
        while max_payout < self.current_payouts.len()
            && self.try_build_current_wallet_psbt(&self.current_payouts[..=max_payout], wallet)?
        {
            max_payout += 1;
        }
        if max_payout == 0 {
            return Ok(self);
        }

        let mut builder = wallet.build_tx();
        builder.fee_rate(self.fee_rate.expect("fee rate must be set"));
        builder.sighash(bitcoin::EcdsaSighashType::All.into());

        for next_payout in self.current_payouts.drain(..max_payout) {
            let destination = next_payout
                .destination
                .onchain_address()
                .expect("No onchain address");

            builder.add_recipient(destination.script_pubkey(), next_payout.satoshis);
            self.included_payouts
                .entry(self.current_wallet.expect("current wallet must be set"))
                .or_default()
                .push(next_payout);
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
                self.included_utxos
                    .entry(keychain_id)
                    .or_default()
                    .push(input.previous_output);
                self.all_included_utxos.insert(input.previous_output);
            }
        }

        if let Some(result_psbt) = self.result_psbt {
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
            Ok((psbt, _details)) => {
                for input in psbt.unsigned_tx.input.iter() {
                    self.input_weights
                        .insert(input.previous_output, keychain_satisfaction_weight);
                    if self.all_included_utxos.insert(input.previous_output) {
                        self.included_utxos
                            .entry(keychain_id)
                            .or_default()
                            .push(input.previous_output);
                    }
                }
                self.result_psbt = Some(psbt);
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
            current_wallet: None,
            current_payouts: vec![],
            current_wallet_psbts: self.current_wallet_psbts,
            included_payouts: self.included_payouts,
            included_utxos: self.included_utxos,
            all_included_utxos: self.all_included_utxos,
            input_weights: self.input_weights,
            result_psbt: self.result_psbt,
            _phantom: PhantomData,
        }
    }

    pub fn finish(self) -> Result<FinishedPsbtBuild, BriaError> {
        Ok(FinishedPsbtBuild {
            included_payouts: self.included_payouts,
            included_utxos: self.included_utxos,
            psbt: self.result_psbt,
        })
    }

    fn try_build_current_wallet_psbt<D: BatchDatabase>(
        &self,
        payouts: &[Payout],
        wallet: &Wallet<D>,
    ) -> Result<bool, BriaError> {
        let mut builder = wallet.build_tx();
        builder.fee_rate(self.fee_rate.expect("fee rate must be set"));

        for next_payout in payouts.iter() {
            let destination = next_payout
                .destination
                .onchain_address()
                .expect("No onchain address");

            builder.add_recipient(destination.script_pubkey(), next_payout.satoshis);
        }

        for (_, psbt) in self.current_wallet_psbts.iter() {
            for (input, psbt_input) in psbt.unsigned_tx.input.iter().zip(psbt.inputs.iter()) {
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
            Ok(_) => Ok(true),
            Err(bdk::Error::InsufficientFunds { .. }) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}

pub struct FinishedPsbtBuild {
    pub included_payouts: HashMap<WalletId, Vec<Payout>>,
    pub included_utxos: HashMap<KeychainId, Vec<OutPoint>>,
    pub psbt: Option<psbt::PartiallySignedTransaction>,
}
