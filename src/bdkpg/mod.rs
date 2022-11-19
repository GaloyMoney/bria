use bdk::{
    database::{BatchDatabase, BatchOperations, Database, SyncTime},
    KeychainKind, LocalUtxo, TransactionDetails,
};
use bitcoin::{blockdata::transaction::OutPoint, Script, Transaction, Txid};

struct SqlxWalletDb {}

impl BatchOperations for SqlxWalletDb {
    fn set_script_pubkey(&mut self, _: &Script, _: KeychainKind, _: u32) -> Result<(), bdk::Error> {
        unimplemented!()
    }
    fn set_utxo(&mut self, _: &LocalUtxo) -> Result<(), bdk::Error> {
        unimplemented!()
    }
    fn set_raw_tx(&mut self, _: &Transaction) -> Result<(), bdk::Error> {
        unimplemented!()
    }
    fn set_tx(&mut self, _: &TransactionDetails) -> Result<(), bdk::Error> {
        unimplemented!()
    }
    fn set_last_index(&mut self, _: KeychainKind, _: u32) -> Result<(), bdk::Error> {
        unimplemented!()
    }
    fn set_sync_time(&mut self, _: SyncTime) -> Result<(), bdk::Error> {
        unimplemented!()
    }
    fn del_script_pubkey_from_path(
        &mut self,
        _: KeychainKind,
        _: u32,
    ) -> Result<Option<Script>, bdk::Error> {
        unimplemented!()
    }
    fn del_path_from_script_pubkey(
        &mut self,
        _: &Script,
    ) -> Result<Option<(KeychainKind, u32)>, bdk::Error> {
        unimplemented!()
    }
    fn del_utxo(&mut self, _: &OutPoint) -> Result<Option<LocalUtxo>, bdk::Error> {
        unimplemented!()
    }
    fn del_raw_tx(&mut self, _: &Txid) -> Result<Option<Transaction>, bdk::Error> {
        unimplemented!()
    }
    fn del_tx(&mut self, _: &Txid, _: bool) -> Result<Option<TransactionDetails>, bdk::Error> {
        unimplemented!()
    }
    fn del_last_index(&mut self, _: KeychainKind) -> Result<std::option::Option<u32>, bdk::Error> {
        unimplemented!()
    }
    fn del_sync_time(&mut self) -> Result<Option<SyncTime>, bdk::Error> {
        unimplemented!()
    }
}

impl Database for SqlxWalletDb {
    fn check_descriptor_checksum<B>(&mut self, _: KeychainKind, _: B) -> Result<(), bdk::Error>
    where
        B: AsRef<[u8]>,
    {
        unimplemented!()
    }
    fn iter_script_pubkeys(&self, _: Option<KeychainKind>) -> Result<Vec<Script>, bdk::Error> {
        unimplemented!()
    }
    fn iter_utxos(&self) -> Result<Vec<LocalUtxo>, bdk::Error> {
        unimplemented!()
    }
    fn iter_raw_txs(&self) -> Result<Vec<Transaction>, bdk::Error> {
        unimplemented!()
    }
    fn iter_txs(&self, _: bool) -> Result<Vec<TransactionDetails>, bdk::Error> {
        unimplemented!()
    }
    fn get_script_pubkey_from_path(
        &self,
        _: KeychainKind,
        _: u32,
    ) -> Result<Option<Script>, bdk::Error> {
        unimplemented!()
    }
    fn get_path_from_script_pubkey(
        &self,
        _: &Script,
    ) -> Result<Option<(KeychainKind, u32)>, bdk::Error> {
        unimplemented!()
    }
    fn get_utxo(&self, _: &OutPoint) -> Result<Option<LocalUtxo>, bdk::Error> {
        unimplemented!()
    }
    fn get_raw_tx(&self, _: &Txid) -> Result<Option<Transaction>, bdk::Error> {
        unimplemented!()
    }
    fn get_tx(&self, _: &Txid, _: bool) -> Result<Option<TransactionDetails>, bdk::Error> {
        unimplemented!()
    }
    fn get_last_index(&self, _: KeychainKind) -> Result<std::option::Option<u32>, bdk::Error> {
        unimplemented!()
    }
    fn get_sync_time(&self) -> Result<Option<SyncTime>, bdk::Error> {
        unimplemented!()
    }
    fn increment_last_index(&mut self, _: KeychainKind) -> Result<u32, bdk::Error> {
        unimplemented!()
    }
}

impl BatchDatabase for SqlxWalletDb {
    type Batch = Self;

    fn begin_batch(&self) -> <Self as BatchDatabase>::Batch {
        unimplemented!()
    }
    fn commit_batch(&mut self, _: <Self as BatchDatabase>::Batch) -> Result<(), bdk::Error> {
        unimplemented!()
    }
}
