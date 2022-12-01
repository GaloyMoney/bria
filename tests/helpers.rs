#![allow(dead_code)]

use bdk::bitcoin::{Address, Amount};
use bitcoincore_rpc::{Client as BitcoindClient, RpcApi};
use bria::{admin::*, primitives::*};
use rand::distributions::{Alphanumeric, DistString};

pub async fn init_pool() -> anyhow::Result<sqlx::PgPool> {
    let pg_host = std::env::var("PG_HOST").unwrap_or("localhost".to_string());
    let pg_con = format!("postgres://user:password@{pg_host}:5432/pg");
    let pool = sqlx::PgPool::connect(&pg_con).await?;
    Ok(pool)
}

pub async fn create_test_account(pool: &sqlx::PgPool) -> anyhow::Result<AccountId> {
    let name = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let app = AdminApp::new(pool.clone());
    Ok(app.create_account(name).await?.account_id)
}

const BITCOIND_WALLET_NAME: &str = "bria";
pub fn bitcoind_client() -> anyhow::Result<bitcoincore_rpc::Client> {
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
