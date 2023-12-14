use anyhow::Context;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

use crate::primitives::bitcoin;

#[derive(Serialize)]
struct AppOutput {
    app: SecurityOutput,
}
#[derive(Serialize)]
struct SecurityOutput {
    security: AddressOutput,
}
#[derive(Serialize)]
struct AddressOutput {
    blocked_addresses: Vec<bitcoin::BdkAddress>,
}

fn extract_addresses(file_contents: &str) -> HashSet<bitcoin::BdkAddress> {
    let re = Regex::new(r"\b(bc1|1|3|tb1|m|n|2)[0-9a-zA-Z]*\b").unwrap();
    let mut addresses: HashSet<bitcoin::BdkAddress> = HashSet::new();

    for cap in re.captures_iter(file_contents) {
        if let Some(addr) = cap.get(0) {
            if let Ok(address) = addr.as_str().parse::<bitcoin::BdkAddress<_>>() {
                addresses.insert(address.assume_checked());
            }
        }
    }
    addresses
}

pub fn read_and_parse_addresses(file_path: impl AsRef<Path>) -> anyhow::Result<()> {
    let s = std::fs::read_to_string(file_path).context("Couldn't read file")?;
    let mut blocked_addresses: Vec<_> = extract_addresses(&s).into_iter().collect();
    blocked_addresses.sort();

    let app_output = AppOutput {
        app: SecurityOutput {
            security: AddressOutput { blocked_addresses },
        },
    };

    let yaml = serde_yaml::to_string(&app_output).unwrap();
    println!("{}", yaml);
    Ok(())
}
