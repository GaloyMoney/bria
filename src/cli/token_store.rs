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

pub fn store_profile_token(bria_home: &str, token: &str) -> anyhow::Result<()> {
    println!("Writing profile token to {bria_home}/profile-api-key");
    create_token_dir(bria_home)?;
    let _ = fs::remove_file(format!("{bria_home}/profile-api-key"));
    fs::write(format!("{bria_home}/profile-api-key"), token).context("Writing Profile API Key")?;
    Ok(())
}

pub fn load_profile_token(bria_home: &str) -> anyhow::Result<String> {
    let token = fs::read_to_string(format!("{bria_home}/profile-api-key"))?;
    Ok(token)
}

pub fn store_daemon_pid(bria_home: &str, pid: u32) -> anyhow::Result<()> {
    create_token_dir(bria_home)?;
    let _ = fs::remove_file(format!("{bria_home}/daemon-pid"));
    fs::write(format!("{bria_home}/daemon-pid"), pid.to_string()).context("Writing PID file")?;
    Ok(())
}

fn create_token_dir(bria_home: &str) -> anyhow::Result<()> {
    let _ = fs::create_dir(bria_home);
    Ok(())
}
