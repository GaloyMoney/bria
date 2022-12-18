use bdk::{database::BatchDatabase, wallet::tx_builder::TxOrdering, FeeRate, LocalUtxo, Wallet};
use bitcoin::util::psbt;
use std::{collections::HashMap, marker::PhantomData};

use super::keychain::*;
use crate::{error::*, payout::Payout, primitives::*};

pub struct PsbtBuilder<T> {
    consolidate_deprecated_keychains: Option<bool>,
    fee_rate: Option<FeeRate>,
    current_wallet: Option<WalletId>,
    current_payouts: Vec<Payout>,
    included_payouts: HashMap<WalletId, Payout>,
    included_utxos: HashMap<KeychainId, Vec<LocalUtxo>>,
    current_wallet_psbt: Option<psbt::PartiallySignedTransaction>,
    final_psbt: Option<psbt::PartiallySignedTransaction>,
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
            current_wallet_psbt: None,
            final_psbt: None,
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
            current_wallet_psbt: None,
            final_psbt: None,
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
            current_wallet_psbt: None,
            final_psbt: None,
            _phantom: PhantomData,
        }
    }
}

pub struct AcceptingKeychainState;

impl BdkWalletVisitor for PsbtBuilder<AcceptingKeychainState> {
    fn visit_bdk_wallet<D: BatchDatabase>(
        mut self,
        _keychain_id: KeychainId,
        wallet: &Wallet<D>,
    ) -> Result<Self, BriaError> {
        let next_payout = &self.current_payouts[0];
        let destination = next_payout
            .destination
            .onchain_address()
            .expect("No onchain address");
        let amount = next_payout.satoshis;

        let mut builder = wallet.build_tx();
        builder.fee_rate(self.fee_rate.expect("fee rate must be set"));
        builder.add_recipient(destination.script_pubkey(), amount);
        builder.ordering(TxOrdering::Bip69Lexicographic);
        // builder.sighash(bitcoin::EcdsaSighashType::All.into());
        builder.sighash(bitcoin::SchnorrSighashType::All.into());
        match builder.finish() {
            Ok((psbt, _details)) => {
                self.current_wallet_psbt = Some(psbt);
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
            current_wallet_psbt: None,
            final_psbt: self.current_wallet_psbt,
            _phantom: PhantomData,
        }
    }

    pub fn finish(self) -> FinishedPsbtBuild {
        FinishedPsbtBuild {
            psbt: self.current_wallet_psbt,
            // Some(
            //     self.final_psbt
            //         .unwrap_or_else(|| self.current_wallet_psbt.expect("psbt must be set")),
            // ),
        }
    }
}

pub struct FinishedPsbtBuild {
    pub psbt: Option<psbt::PartiallySignedTransaction>,
}
