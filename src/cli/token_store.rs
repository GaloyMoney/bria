use std::fs;

use anyhow::Context;

pub fn load_admin_token(bria_home: &str) -> anyhow::Result<String> {
    let token = fs::read_to_string(format!("{bria_home}/admin-api-key"))?;
    Ok(token)
}

pub fn store_admin_token(bria_home: &str, token: &str) -> anyhow::Result<()> {
    println!("Writing admin token to {bria_home}/admin-api-key");
    create_token_dir(bria_home)?;
    let _ = fs::remove_file(format!("{bria_home}/admin-api-key"));
    fs::write(format!("{bria_home}/admin-api-key"), token).context("Writing Admin API Key")?;
    Ok(())
}

pub fn store_account_token(bria_home: &str, token: &str) -> anyhow::Result<()> {
    println!("Writing account token to {bria_home}/account-api-key");
    create_token_dir(bria_home)?;
    let _ = fs::remove_file(format!("{bria_home}/account-api-key"));
    fs::write(format!("{bria_home}/account-api-key"), token).context("Writing Account API Key")?;
    Ok(())
}

pub fn load_account_token(bria_home: &str) -> anyhow::Result<String> {
    let token = fs::read_to_string(format!("{bria_home}/account-api-key"))?;
    Ok(token)
}

pub fn store_daemon_pid(bria_home: &str, pid: u32) -> anyhow::Result<()> {
    create_token_dir(bria_home)?;
    let _ = fs::remove_file(format!("{bria_home}/daemon_pid"));
    fs::write(format!("{bria_home}/daemon_pid"), pid.to_string()).context("Writing PID file")?;
    Ok(())
}

fn create_token_dir(bria_home: &str) -> anyhow::Result<()> {
    let _ = fs::create_dir(bria_home);
    Ok(())
}
