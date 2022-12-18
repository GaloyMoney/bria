#![allow(dead_code)]

use bdk::descriptor::IntoWalletDescriptor;
use bdk::miniscript::Tap;
use bdk::{
    bitcoin::{Address, Amount},
    blockchain::ElectrumBlockchain,
    database::MemoryDatabase,
    electrum_client::Client,
    keys::{GeneratableKey, GeneratedKey, PrivateKeyGenerateOptions},
};
use bitcoin::{
    secp256k1::{rand, Secp256k1},
    Network, PrivateKey,
};
use bitcoincore_rpc::{Client as BitcoindClient, RpcApi};
use bria::{admin::*, primitives::*, signer::*};
use rand::distributions::{Alphanumeric, DistString};

pub async fn init_pool() -> anyhow::Result<sqlx::PgPool> {
    let pg_host = std::env::var("PG_HOST").unwrap_or("localhost".to_string());
    let pg_con = format!("postgres://user:password@{pg_host}:5432/pg");
    let pool = sqlx::PgPool::connect(&pg_con).await?;
    Ok(pool)
}

pub async fn create_test_account(pool: &sqlx::PgPool) -> anyhow::Result<AccountId> {
    let name = format!(
        "TEST_{}",
        Alphanumeric.sample_string(&mut rand::thread_rng(), 32)
    );
    let app = AdminApp::new(pool.clone());

    Ok(app.create_account(name).await?.account_id)
}

const BITCOIND_WALLET_NAME: &str = "bria";
pub fn bitcoind_client() -> anyhow::Result<bitcoincore_rpc::Client> {
    match bitcoind_client_inner() {
        Err(_) => bitcoind_client_inner(),
        Ok(c) => Ok(c),
    }
}
pub fn bitcoind_client_inner() -> anyhow::Result<bitcoincore_rpc::Client> {
    use bitcoincore_rpc::Auth;

    let bitcoind_host = std::env::var("BITCIOND_HOST").unwrap_or("localhost".to_string());
    let client = BitcoindClient::new(
        &format!("{bitcoind_host}:18443"),
        Auth::UserPass("rpcuser".to_string(), "rpcpassword".to_string()),
    )?;
    if client.list_wallets()?.is_empty() {
        client.create_wallet(BITCOIND_WALLET_NAME, None, None, None, None)?;
        let addr = client.get_new_address(None, None)?;
        client.generate_to_address(101, &addr)?;
    }
    let wallet_info = client.get_wallet_info()?;
    if wallet_info.wallet_name != BITCOIND_WALLET_NAME {
        client.create_wallet(BITCOIND_WALLET_NAME, None, None, None, None)?;
        let addr = client.get_new_address(None, None)?;
        client.generate_to_address(101, &addr)?;
    }
    Ok(client)
}

pub async fn lnd_signing_client() -> anyhow::Result<LndRemoteSigner> {
    let macaroon_base64 = read_to_base64("./dev/lnd/regtest/lnd.admin.macaroon")?;
    let cert_base64 = read_to_base64("./dev/lnd/tls.cert")?;
    let cfg = LndSignerConfig {
        endpoint: "https://localhost:10009".to_string(),
        macaroon_base64,
        cert_base64,
    };
    Ok(LndRemoteSigner::connect(cfg).await?)
}

pub fn fund_addr(bitcoind: &BitcoindClient, addr: &Address, amount: u32) -> anyhow::Result<()> {
    let fund = bitcoind.get_new_address(None, None)?;
    bitcoind.generate_to_address(6, &fund)?;
    bitcoind.send_to_address(
        &addr,
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

pub fn electrum_blockchain() -> anyhow::Result<ElectrumBlockchain> {
    let electrum_host = std::env::var("ELECTRUM_HOST").unwrap_or("localhost".to_string());
    let electrum_url = format!("{electrum_host}:50001");
    Ok(ElectrumBlockchain::from(Client::new(&electrum_url)?))
}

pub fn random_bdk_wallet() -> anyhow::Result<()> {
    let secp = Secp256k1::new();
    let sk: GeneratedKey<PrivateKey, Tap> =
        PrivateKey::generate(PrivateKeyGenerateOptions::default())?;
    let pubkey = sk.public_key(&secp);
    let wallet = bdk::Wallet::new(
        format!("wpkh({})", pubkey).into_wallet_descriptor(&secp, Network::Regtest)?,
        None,
        bitcoin::Network::Regtest,
        MemoryDatabase::new(),
    );
    // Ok(wallet)
    Ok(())
}

fn read_to_base64(path: impl Into<std::path::PathBuf>) -> anyhow::Result<String> {
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Read;
    let f = File::open(path.into())?;
    let mut reader = BufReader::new(f);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    Ok(base64::encode(buffer))
}
