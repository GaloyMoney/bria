mod convert;
mod descriptor_checksum;
mod index;
mod script_pubkeys;
mod sync_times;
mod transactions;
mod utxos;

use bdk::{
    bitcoin::{blockdata::transaction::OutPoint, Script, ScriptBuf, Transaction, Txid},
    database::{BatchDatabase, BatchOperations, Database, SyncTime},
    KeychainKind, LocalUtxo, TransactionDetails,
};
use sqlx::PgPool;
use tokio::runtime::Handle;

use crate::primitives::*;
use convert::BdkKeychainKind;
use descriptor_checksum::DescriptorChecksums;
use index::Indexes;
use script_pubkeys::ScriptPubkeys;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
pub(super) use sync_times::SyncTimes;
pub use transactions::*;
pub use utxos::*;

pub struct SqlxWalletDb {
    rt: Handle,
    pool: PgPool,
    keychain_id: KeychainId,
    utxos: Option<Vec<LocalUtxo>>,
    cached_spks: Arc<Mutex<HashMap<ScriptBuf, (KeychainKind, u32)>>>,
    addresses: HashMap<ScriptBuf, (KeychainKind, u32)>,
    cached_txs: Arc<Mutex<HashMap<Txid, TransactionDetails>>>,
    txs: HashMap<Txid, TransactionDetails>,
}

impl SqlxWalletDb {
    pub fn new(pool: PgPool, keychain_id: KeychainId) -> Self {
        Self {
            rt: Handle::current(),
            keychain_id,
            pool,
            utxos: None,
            addresses: HashMap::new(),
            cached_spks: Arc::new(Mutex::new(HashMap::new())),
            txs: HashMap::new(),
            cached_txs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn load_all_txs(&self) -> Result<(), bdk::Error> {
        let mut txs = self.cached_txs.lock().expect("poisoned txs cache lock");
        if txs.is_empty() {
            let loaded = self.rt.block_on(async {
                let txs = Transactions::new(self.keychain_id, self.pool.clone());
                txs.load_all().await
            })?;
            *txs = loaded;
        }
        Ok(())
    }

    fn lookup_tx(&self, txid: &Txid) -> Result<Option<TransactionDetails>, bdk::Error> {
        if let Some(tx) = self.txs.get(txid) {
            return Ok(Some(tx.clone()));
        }
        self.load_all_txs()?;
        Ok(self
            .cached_txs
            .lock()
            .expect("poisoned txs cache lock")
            .get(txid)
            .cloned())
    }
}

impl BatchOperations for SqlxWalletDb {
    fn set_script_pubkey(
        &mut self,
        script: &Script,
        keychain: KeychainKind,
        path: u32,
    ) -> Result<(), bdk::Error> {
        self.addresses.insert(script.into(), (keychain, path));
        Ok(())
    }

    fn set_utxo(&mut self, utxo: &LocalUtxo) -> Result<(), bdk::Error> {
        if self.utxos.is_none() {
            self.utxos = Some(Vec::new());
        }
        self.utxos.as_mut().unwrap().push(utxo.clone());
        Ok(())
    }

    fn set_raw_tx(&mut self, _: &Transaction) -> Result<(), bdk::Error> {
        unimplemented!()
    }

    fn set_tx(&mut self, tx: &TransactionDetails) -> Result<(), bdk::Error> {
        self.txs.insert(tx.txid, tx.clone());
        Ok(())
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
    ) -> Result<Option<ScriptBuf>, bdk::Error> {
        unimplemented!()
    }
    fn del_path_from_script_pubkey(
        &mut self,
        _: &Script,
    ) -> Result<Option<(KeychainKind, u32)>, bdk::Error> {
        unimplemented!()
    }
    fn del_utxo(&mut self, outpoint: &OutPoint) -> Result<Option<LocalUtxo>, bdk::Error> {
        self.rt.block_on(async {
            Utxos::new(self.keychain_id, self.pool.clone())
                .delete(outpoint)
                .await
        })
    }
    fn del_raw_tx(&mut self, _: &Txid) -> Result<Option<Transaction>, bdk::Error> {
        unimplemented!()
    }

    fn del_tx(
        &mut self,
        tx_id: &Txid,
        _include_raw: bool,
    ) -> Result<Option<TransactionDetails>, bdk::Error> {
        self.rt.block_on(async {
            let txs = Transactions::new(self.keychain_id, self.pool.clone());
            txs.delete(tx_id).await
        })
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
    ) -> Result<Vec<ScriptBuf>, bdk::Error> {
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
        self.load_all_txs()?;
        Ok(self
            .cached_txs
            .lock()
            .expect("poisoned txs cache lock")
            .values()
            .cloned()
            .collect())
    }

    fn get_script_pubkey_from_path(
        &self,
        keychain: KeychainKind,
        path: u32,
    ) -> Result<Option<ScriptBuf>, bdk::Error> {
        self.rt.block_on(async {
            let script_pubkeys = ScriptPubkeys::new(self.keychain_id, self.pool.clone());
            script_pubkeys.find_script(keychain, path).await
        })
    }
    fn get_path_from_script_pubkey(
        &self,
        script: &Script,
    ) -> Result<Option<(KeychainKind, u32)>, bdk::Error> {
        let mut cache = self.cached_spks.lock().expect("poisoned spk cache lock");
        if cache.is_empty() {
            let loaded = self.rt.block_on(async {
                let script_pubkeys = ScriptPubkeys::new(self.keychain_id, self.pool.clone());
                script_pubkeys.load_all().await
            })?;
            *cache = loaded;
        }

        if let Some(res) = cache.get(script) {
            Ok(Some(*res))
        } else if let Some(res) = self.addresses.get(script) {
            Ok(Some(*res))
        } else {
            Ok(None)
        }
    }
    fn get_utxo(&self, outpoint: &OutPoint) -> Result<Option<LocalUtxo>, bdk::Error> {
        self.rt.block_on(async {
            Utxos::new(self.keychain_id, self.pool.clone())
                .find(outpoint)
                .await
        })
    }
    fn get_raw_tx(&self, tx_id: &Txid) -> Result<Option<Transaction>, bdk::Error> {
        self.lookup_tx(tx_id)
            .map(|tx| tx.and_then(|tx| tx.transaction))
    }
    fn get_tx(
        &self,
        tx_id: &Txid,
        _include_raw: bool,
    ) -> Result<Option<TransactionDetails>, bdk::Error> {
        self.lookup_tx(tx_id)
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
        let mut res = SqlxWalletDb::new(self.pool.clone(), self.keychain_id);
        res.cached_spks = Arc::clone(&self.cached_spks);
        res.cached_txs = Arc::clone(&self.cached_txs);
        res
    }

    fn commit_batch(
        &mut self,
        mut batch: <Self as BatchDatabase>::Batch,
    ) -> Result<(), bdk::Error> {
        self.cached_spks
            .lock()
            .expect("poisoned spk cache lock")
            .extend(
                batch
                    .addresses
                    .iter()
                    .map(|(s, (k, p))| (s.clone(), (*k, *p))),
            );

        self.cached_txs
            .lock()
            .expect("poisoned txs cache lock")
            .extend(batch.txs.iter().map(|(id, tx)| (*id, tx.clone())));

        self.rt.block_on(async move {
            if !batch.addresses.is_empty() {
                let addresses: Vec<_> = batch
                    .addresses
                    .drain()
                    .map(|(s, (k, p))| (BdkKeychainKind::from(k), p, s))
                    .collect();
                let repo = ScriptPubkeys::new(batch.keychain_id, batch.pool.clone());
                repo.persist_all(addresses).await?;
            }

            if let Some(utxos) = batch.utxos.take() {
                let repo = Utxos::new(batch.keychain_id, batch.pool.clone());
                repo.persist_all(utxos).await?;
            }
            if !batch.txs.is_empty() {
                let txs = batch.txs.drain().map(|(_, tx)| tx).collect();
                let repo = Transactions::new(batch.keychain_id, batch.pool.clone());
                repo.persist_all(txs).await?;
            }
            Ok::<_, bdk::Error>(())
        })
    }
}
