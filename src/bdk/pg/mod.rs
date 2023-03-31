mod convert;
mod descriptor_checksum;
mod index;
mod script_pubkeys;
mod sync_times;
mod transactions;
mod utxos;

use bdk::{
    bitcoin::{blockdata::transaction::OutPoint, Script, Transaction, Txid},
    database::{BatchDatabase, BatchOperations, Database, SyncTime},
    KeychainKind, LocalUtxo, TransactionDetails,
};
use sqlx::PgPool;
use tokio::runtime::Handle;

use crate::primitives::*;
use descriptor_checksum::DescriptorChecksums;
use index::Indexes;
use script_pubkeys::ScriptPubkeys;
use sync_times::SyncTimes;
pub use transactions::*;
pub use utxos::*;

pub struct SqlxWalletDb {
    rt: Handle,
    pool: PgPool,
    keychain_id: KeychainId,
}

impl SqlxWalletDb {
    pub fn new(pool: PgPool, keychain_id: KeychainId) -> Self {
        Self {
            rt: Handle::current(),
            keychain_id,
            pool,
        }
    }
}

impl BatchOperations for SqlxWalletDb {
    fn set_script_pubkey(
        &mut self,
        script: &Script,
        keychain: KeychainKind,
        path: u32,
    ) -> Result<(), bdk::Error> {
        self.rt.block_on(async {
            let script_pubkeys = ScriptPubkeys::new(self.keychain_id, self.pool.clone());
            script_pubkeys.persist(keychain, path, script).await?;
            Ok(())
        })
    }

    fn set_utxo(&mut self, utxo: &LocalUtxo) -> Result<(), bdk::Error> {
        self.rt.block_on(async {
            Utxos::new(self.keychain_id, self.pool.clone())
                .persist(utxo)
                .await
        })
    }

    fn set_raw_tx(&mut self, _: &Transaction) -> Result<(), bdk::Error> {
        unimplemented!()
    }
    fn set_tx(&mut self, tx: &TransactionDetails) -> Result<(), bdk::Error> {
        self.rt.block_on(async {
            let txs = Transactions::new(self.keychain_id, self.pool.clone());
            txs.persist(tx).await
        })
    }
    fn set_last_index(&mut self, kind: KeychainKind, idx: u32) -> Result<(), bdk::Error> {
        self.rt.block_on(async {
            let indexes = Indexes::new(self.keychain_id, self.pool.clone());
            indexes.persist_last_index(kind, idx).await
        })
    }
    fn set_sync_time(&mut self, time: SyncTime) -> Result<(), bdk::Error> {
        self.rt.block_on(async {
            let sync_times = SyncTimes::new(self.keychain_id, self.pool.clone());
            sync_times.persist(time).await
        })
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
    fn check_descriptor_checksum<B>(
        &mut self,
        keychain: KeychainKind,
        script_bytes: B,
    ) -> Result<(), bdk::Error>
    where
        B: AsRef<[u8]>,
    {
        self.rt.block_on(async {
            let checksums = DescriptorChecksums::new(self.keychain_id, self.pool.clone());
            checksums
                .check_or_persist_descriptor_checksum(keychain, script_bytes.as_ref())
                .await?;

            Ok(())
        })
    }
    fn iter_script_pubkeys(
        &self,
        keychain: Option<KeychainKind>,
    ) -> Result<Vec<Script>, bdk::Error> {
        self.rt.block_on(async {
            let script_pubkeys = ScriptPubkeys::new(self.keychain_id, self.pool.clone());
            let scripts = script_pubkeys.list_scripts(keychain).await?;
            Ok(scripts)
        })
    }
    fn iter_utxos(&self) -> Result<Vec<LocalUtxo>, bdk::Error> {
        self.rt.block_on(async {
            Utxos::new(self.keychain_id, self.pool.clone())
                .list_local_utxos()
                .await
        })
    }
    fn iter_raw_txs(&self) -> Result<Vec<Transaction>, bdk::Error> {
        unimplemented!()
    }

    fn iter_txs(&self, _: bool) -> Result<Vec<TransactionDetails>, bdk::Error> {
        self.rt.block_on(async {
            let txs = Transactions::new(self.keychain_id, self.pool.clone());
            txs.list().await
        })
    }

    fn get_script_pubkey_from_path(
        &self,
        keychain: KeychainKind,
        path: u32,
    ) -> Result<Option<Script>, bdk::Error> {
        self.rt.block_on(async {
            let script_pubkeys = ScriptPubkeys::new(self.keychain_id, self.pool.clone());
            script_pubkeys.find_script(keychain, path).await
        })
    }
    fn get_path_from_script_pubkey(
        &self,
        script: &Script,
    ) -> Result<Option<(KeychainKind, u32)>, bdk::Error> {
        self.rt.block_on(async {
            let script_pubkeys = ScriptPubkeys::new(self.keychain_id, self.pool.clone());
            Ok(script_pubkeys
                .find_path(script)
                .await?
                .map(|(kind, path)| (kind.into(), path)))
        })
    }
    fn get_utxo(&self, _: &OutPoint) -> Result<Option<LocalUtxo>, bdk::Error> {
        unimplemented!()
    }
    fn get_raw_tx(&self, tx_id: &Txid) -> Result<Option<Transaction>, bdk::Error> {
        self.rt.block_on(async {
            let txs = Transactions::new(self.keychain_id, self.pool.clone());
            Ok(txs.find_by_id(tx_id).await?.and_then(|tx| tx.transaction))
        })
    }
    fn get_tx(
        &self,
        tx_id: &Txid,
        _include_raw: bool,
    ) -> Result<Option<TransactionDetails>, bdk::Error> {
        self.rt.block_on(async {
            let txs = Transactions::new(self.keychain_id, self.pool.clone());
            txs.find_by_id(tx_id).await
        })
    }
    fn get_last_index(&self, kind: KeychainKind) -> Result<std::option::Option<u32>, bdk::Error> {
        self.rt.block_on(async {
            let last_indexes = Indexes::new(self.keychain_id, self.pool.clone());
            last_indexes.get_latest(kind).await
        })
    }
    fn get_sync_time(&self) -> Result<Option<SyncTime>, bdk::Error> {
        self.rt.block_on(async {
            let sync_times = SyncTimes::new(self.keychain_id, self.pool.clone());
            sync_times.get().await
        })
    }
    fn increment_last_index(&mut self, keychain: KeychainKind) -> Result<u32, bdk::Error> {
        self.rt.block_on(async {
            let indexes = Indexes::new(self.keychain_id, self.pool.clone());
            indexes.increment(keychain).await
        })
    }
}

impl BatchDatabase for SqlxWalletDb {
    type Batch = Self;

    fn begin_batch(&self) -> <Self as BatchDatabase>::Batch {
        SqlxWalletDb::new(self.pool.clone(), self.keychain_id)
    }
    fn commit_batch(&mut self, _: <Self as BatchDatabase>::Batch) -> Result<(), bdk::Error> {
        Ok(())
    }
}
