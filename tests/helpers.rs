#![allow(dead_code)]

use anyhow::Context;
use bdk::{
    bitcoin::{
        secp256k1::{rand, Secp256k1},
        Address, Amount, PrivateKey,
    },
    blockchain::ElectrumBlockchain,
    database::MemoryDatabase,
    descriptor::IntoWalletDescriptor,
    electrum_client::{Client, ConfigBuilder},
    keys::{GeneratableKey, GeneratedKey, PrivateKeyGenerateOptions},
    miniscript::Segwitv0,
};
use bitcoincore_rpc::{Client as BitcoindClient, RpcApi};
use bria::{admin::*, primitives::*, profile::*, xpub::*};
use rand::distributions::{Alphanumeric, DistString};

pub async fn init_pool() -> anyhow::Result<sqlx::PgPool> {
    let pg_host = std::env::var("PG_HOST").unwrap_or("localhost".to_string());
    let pg_con = format!("postgres://user:password@{pg_host}:5432/pg");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(&pg_con)
        .await?;
    Ok(pool)
}

pub async fn create_test_account(pool: &sqlx::PgPool) -> anyhow::Result<Profile> {
    let name = format!(
        "TEST_{}",
        Alphanumeric.sample_string(&mut rand::thread_rng(), 32)
    );
    let app = AdminApp::new(pool.clone(), bitcoin::Network::Regtest);

    let profile_key = app.create_account(name.clone()).await?;
    Ok(Profile {
        id: profile_key.profile_id,
        account_id: profile_key.account_id,
        name,
    })
}

const BITCOIND_WALLET_NAME: &str = "bria";
pub fn bitcoind_client() -> anyhow::Result<bitcoincore_rpc::Client> {
    match bitcoind_client_inner() {
        Err(e) => {
            dbg!("bitcoind_client_inner failed: {}", e);
            let _ = std::fs::remove_dir_all("/data/.bitcoin/regtest/wallets/bria");
            bitcoind_client_inner()
        }
        Ok(c) => Ok(c),
    }
}
pub fn bitcoind_client_inner() -> anyhow::Result<bitcoincore_rpc::Client> {
    use bitcoincore_rpc::Auth;

    let bitcoind_host = std::env::var("BITCOIND_HOST").unwrap_or("localhost".to_string());
    let client = BitcoindClient::new(
        &format!("{bitcoind_host}:18443"),
        Auth::UserPass("rpcuser".to_string(), "rpcpassword".to_string()),
    )
    .context("BitcoindClient::new")?;
    if client
        .list_wallets()
        .context("client.list_wallets")?
        .is_empty()
    {
        if client.load_wallet(BITCOIND_WALLET_NAME).is_err() {
            client
                .create_wallet(BITCOIND_WALLET_NAME, None, None, None, None)
                .context("client.create_wallet - 1")?;
        }
        let addr = client
            .get_new_address(None, None)
            .context("client.get_new_address - 1")?;
        client
            .generate_to_address(101, &addr)
            .context("client.generate_to_address - 1")?;
    }
    let wallet_info = client.get_wallet_info().context("client.get_wallet_info")?;
    if wallet_info.wallet_name != BITCOIND_WALLET_NAME {
        if client.load_wallet(BITCOIND_WALLET_NAME).is_err() {
            client
                .create_wallet(BITCOIND_WALLET_NAME, None, None, None, None)
                .context("client.create_wallet - 1")?;
        }
        let addr = client
            .get_new_address(None, None)
            .context("client.get_new_address - 2")?;
        client
            .generate_to_address(101, &addr)
            .context("client.generate_to_address - 2")?;
    }
    Ok(client)
}

pub async fn lnd_signing_client() -> anyhow::Result<LndRemoteSigner> {
    let macaroon_base64 = read_to_base64("./dev/lnd/regtest/lnd.admin.macaroon")?;
    let cert_base64 = read_to_base64("./dev/lnd/tls.cert")?;
    let lnd_host = std::env::var("LND_HOST").unwrap_or("localhost".to_string());
    let cfg = LndSignerConfig {
        endpoint: format!("https://{lnd_host}:10009"),
        macaroon_base64,
        cert_base64,
    };
    Ok(LndRemoteSigner::connect(&cfg).await?)
}

pub fn fund_addr(bitcoind: &BitcoindClient, addr: &Address, amount: u32) -> anyhow::Result<()> {
    let fund = bitcoind.get_new_address(None, None)?;
    bitcoind.generate_to_address(6, &fund)?;
    bitcoind.send_to_address(
        addr,
        Amount::from_btc(amount as f64).unwrap(),
        None,
        None,
        None,
        None,
        None,
        None,
    )?;
    Ok(())
}

pub fn gen_blocks(bitcoind: &BitcoindClient, n: u64) -> anyhow::Result<()> {
    let addr = bitcoind.get_new_address(None, None)?;
    bitcoind.generate_to_address(n, &addr)?;
    Ok(())
}

pub async fn electrum_blockchain() -> anyhow::Result<ElectrumBlockchain> {
    let electrum_host = std::env::var("ELECTRUM_HOST").unwrap_or("localhost".to_string());
    let electrum_url = format!("{electrum_host}:50001");

    let cfg = ConfigBuilder::new().retry(10).timeout(Some(4))?.build();
    let mut retries = 0;

    loop {
        match Client::from_config(&electrum_url, cfg.clone()) {
            Ok(client) => {
                return Ok(ElectrumBlockchain::from(client));
            }
            Err(err) if retries >= 10 => {
                return Err(err.into());
            }
            _ => {
                retries += 1;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

pub fn random_bdk_wallet() -> anyhow::Result<bdk::Wallet<MemoryDatabase>> {
    let secp = Secp256k1::new();
    let sk: GeneratedKey<PrivateKey, Segwitv0> =
        PrivateKey::generate(PrivateKeyGenerateOptions::default())?;
    let wallet = bdk::Wallet::new(
        format!("wpkh({})", sk.into_key())
            .into_wallet_descriptor(&secp, bitcoin::Network::Regtest)?,
        None,
        bitcoin::Network::Regtest,
        MemoryDatabase::new(),
    )?;
    Ok(wallet)
}

fn read_to_base64(path: impl Into<std::path::PathBuf>) -> anyhow::Result<String> {
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Read;
    let f = File::open(path.into())?;
    let mut reader = BufReader::new(f);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    use base64::{engine::general_purpose, Engine};
    Ok(general_purpose::STANDARD.encode(buffer))
}
