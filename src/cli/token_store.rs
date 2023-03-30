use std::fs;

use anyhow::Context;

pub fn load_admin_token() -> anyhow::Result<String> {
    let token = fs::read_to_string(".bria/admin-api-key")?;
    Ok(token)
}

pub fn store_admin_token(token: &str) -> anyhow::Result<()> {
    println!("Writing admin token to .bria/admin-api-key");
    create_token_dir()?;
    let _ = fs::remove_file(".bria/admin-api-key");
    fs::write(".bria/admin-api-key", token).context("Writing Admin API Key")?;
    Ok(())
}

pub fn store_account_token(token: &str) -> anyhow::Result<()> {
    println!("Writing account token to .bria/account-api-key");
    create_token_dir()?;
    let _ = fs::remove_file(".bria/account-api-key");
    fs::write(".bria/account-api-key", token).context("Writing Account API Key")?;
    Ok(())
}

pub fn load_account_token() -> anyhow::Result<String> {
    let token = fs::read_to_string(".bria/account-api-key")?;
    Ok(token)
}

pub fn store_daemon_pid(pid: u32) -> anyhow::Result<()> {
    create_token_dir()?;
    let _ = fs::remove_file(".bria/daemon_pid");
    fs::write(".bria/daemon_pid", pid.to_string()).context("Writing PID file")?;
    Ok(())
}

fn create_token_dir() -> anyhow::Result<()> {
    fs::create_dir(".bria").context("Creating token directory")
}
