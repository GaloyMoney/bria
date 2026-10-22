use bdk::blockchain::{ElectrumBlockchain, GetHeight};
use electrum_client::{Client, ConfigBuilder};

use std::sync::{Arc, Mutex};

use crate::bdk::error::BdkError;

/// One Electrum connection shared by all jobs.
#[derive(Clone)]
pub struct SharedElectrum {
    url: Arc<str>,
    client: Arc<Mutex<Option<Arc<ElectrumBlockchain>>>>,
}

impl std::fmt::Debug for SharedElectrum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedElectrum")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl SharedElectrum {
    pub fn new(url: impl AsRef<str>) -> Self {
        Self {
            url: Arc::from(url.as_ref()),
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Returns shared connection
    pub fn get(&self) -> Result<Arc<ElectrumBlockchain>, BdkError> {
        let mut guard = self.client.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(blockchain) = guard.as_ref() {
            return Ok(Arc::clone(blockchain));
        }
        let blockchain = Arc::new(ElectrumBlockchain::from(Client::from_config(
            &self.url,
            ConfigBuilder::new().retry(10).timeout(Some(60)).build(),
        )?));
        *guard = Some(Arc::clone(&blockchain));
        Ok(blockchain)
    }

    /// Return shared connection with block height
    pub fn get_with_height(&self) -> Result<(Arc<ElectrumBlockchain>, u32), BdkError> {
        let blockchain = self.get()?;
        let height = blockchain.get_height()?;
        Ok((blockchain, height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // A fake server that only counts how many connections it receives.
    fn start_counting_server() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = listener.local_addr().unwrap().to_string();
        let count = Arc::new(AtomicUsize::new(0));
        let counter = count.clone();
        std::thread::spawn(move || {
            let mut open = Vec::new();
            for stream in listener.incoming() {
                open.push(stream.unwrap());
                counter.fetch_add(1, Ordering::SeqCst);
            }
        });
        (url, count)
    }

    #[test]
    fn get_reuses_one_connection() {
        let (url, count) = start_counting_server();
        let electrum = SharedElectrum::new(url);

        let first = electrum.get().unwrap();
        for _ in 0..5 {
            let again = electrum.get().unwrap();
            assert!(Arc::ptr_eq(&first, &again));
        }

        std::thread::sleep(std::time::Duration::from_millis(200));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
