use std::fs;

pub fn load_admin_token() -> anyhow::Result<String> {
    let token = fs::read_to_string(".bria/admin-api-key")?;
    Ok(token)
}

pub fn store_admin_token(token: &str) -> anyhow::Result<()> {
    println!("Writing admin token to .bria/admin-api-key");
    create_token_dir()?;
    let _ = fs::remove_file(".bria/admin-api-key");
    fs::write(".bria/admin-api-key", token)?;
    Ok(())
}

pub fn store_account_token(token: &str) -> anyhow::Result<()> {
    println!("Writing account token to .bria/account-api-key");
    create_token_dir()?;
    let _ = fs::remove_file(".bria/account-api-key");
    fs::write(".bria/account-api-key", token)?;
    Ok(())
}

pub fn load_account_token() -> anyhow::Result<String> {
    let token = fs::read_to_string(".bria/account-api-key")?;
    Ok(token)
}

fn create_token_dir() -> anyhow::Result<()> {
    let _ = fs::create_dir(".bria");
    Ok(())
}
