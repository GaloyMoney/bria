use bdk::{
    database::BatchDatabase, wallet::tx_builder::TxOrdering, FeeRate, KeychainKind, LocalUtxo,
    Wallet,
};
use bitcoin::util::psbt;
use std::{collections::HashMap, marker::PhantomData};

use super::keychain::*;
use crate::{error::*, payout::Payout, primitives::*};

struct KeychainData {
    max_satisfaction_weight: usize,
}

pub struct PsbtBuilder<T> {
    consolidate_deprecated_keychains: Option<bool>,
    fee_rate: Option<FeeRate>,
    current_wallet: Option<WalletId>,
    current_payouts: Vec<Payout>,
    included_payouts: HashMap<WalletId, Payout>,
    included_utxos: HashMap<KeychainId, Vec<LocalUtxo>>,
    keychain_data_cache: HashMap<KeychainId, KeychainData>,
    result_psbt: Option<psbt::PartiallySignedTransaction>,
    _phantom: PhantomData<T>,
}

pub struct InitialPsbtBuilderState;

impl PsbtBuilder<InitialPsbtBuilderState> {
    pub fn new() -> Self {
        Self {
            consolidate_deprecated_keychains: None,
            fee_rate: None,
            current_wallet: None,
            current_payouts: vec![],
            included_payouts: HashMap::new(),
            included_utxos: HashMap::new(),
            keychain_data_cache: HashMap::new(),
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

    pub fn begin_wallets(self) -> PsbtBuilder<AcceptingWalletState> {
        PsbtBuilder::<AcceptingWalletState> {
            consolidate_deprecated_keychains: self.consolidate_deprecated_keychains,
            fee_rate: self.fee_rate,
            current_wallet: None,
            current_payouts: vec![],
            included_payouts: self.included_payouts,
            included_utxos: self.included_utxos,
            keychain_data_cache: self.keychain_data_cache,
            result_psbt: self.result_psbt,
            _phantom: PhantomData,
        }
    }
}

pub struct AcceptingWalletState;

impl PsbtBuilder<AcceptingWalletState> {
    pub fn wallet_payouts(
        self,
        wallet_id: WalletId,
        payouts: Vec<Payout>,
    ) -> PsbtBuilder<AcceptingKeychainState> {
        PsbtBuilder::<AcceptingKeychainState> {
            consolidate_deprecated_keychains: self.consolidate_deprecated_keychains,
            fee_rate: self.fee_rate,
            current_wallet: Some(wallet_id),
            current_payouts: payouts,
            included_payouts: self.included_payouts,
            included_utxos: self.included_utxos,
            keychain_data_cache: self.keychain_data_cache,
            result_psbt: self.result_psbt,
            _phantom: PhantomData,
        }
    }
}

pub struct AcceptingKeychainState;

impl BdkWalletVisitor for PsbtBuilder<AcceptingKeychainState> {
    fn visit_bdk_wallet<D: BatchDatabase>(
        mut self,
        keychain_id: KeychainId,
        wallet: &Wallet<D>,
    ) -> Result<Self, BriaError> {
        let keychain_satisfaction_weight = wallet
            .get_descriptor_for_keychain(KeychainKind::External)
            .max_satisfaction_weight()
            .expect("Unsupported descriptor");
        self.keychain_data_cache.insert(
            keychain_id,
            KeychainData {
                max_satisfaction_weight: keychain_satisfaction_weight,
            },
        );

        let next_payout = &self.current_payouts[0];
        let destination = next_payout
            .destination
            .onchain_address()
            .expect("No onchain address");
        let amount = next_payout.satoshis;

        let mut builder = wallet.build_tx();
        builder.fee_rate(self.fee_rate.expect("fee rate must be set"));
        builder.add_recipient(destination.script_pubkey(), amount);
        builder.sighash(bitcoin::EcdsaSighashType::All.into());

        if let Some(result_psbt) = self.result_psbt {
            for (input, psbt_input) in result_psbt
                .unsigned_tx
                .input
                .into_iter()
                .zip(result_psbt.inputs)
            {
                builder.add_foreign_utxo(input.previous_output, psbt_input, 4 + 1 + 73 + 34)?;
            }

            for out in result_psbt.unsigned_tx.output {
                builder.add_recipient(out.script_pubkey, out.value);
            }
        }

        builder.ordering(TxOrdering::Bip69Lexicographic);
        match builder.finish() {
            Ok((psbt, _details)) => {
                self.result_psbt = Some(psbt);
                Ok(self)
            }
            Err(_) => {
                unimplemented!()
            }
        }
    }
}

impl PsbtBuilder<AcceptingKeychainState> {
    pub fn next_wallet(self) -> PsbtBuilder<AcceptingWalletState> {
        PsbtBuilder::<AcceptingWalletState> {
            consolidate_deprecated_keychains: self.consolidate_deprecated_keychains,
            fee_rate: self.fee_rate,
            current_wallet: None,
            current_payouts: vec![],
            included_payouts: self.included_payouts,
            included_utxos: self.included_utxos,
            keychain_data_cache: self.keychain_data_cache,
            result_psbt: self.result_psbt,
            _phantom: PhantomData,
        }
    }

    pub fn finish(self) -> Result<FinishedPsbtBuild, BriaError> {
        Ok(FinishedPsbtBuild {
            psbt: self.result_psbt,
        })
    }
}

pub struct FinishedPsbtBuild {
    pub psbt: Option<psbt::PartiallySignedTransaction>,
}
