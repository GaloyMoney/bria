use anyhow::Context;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

use crate::primitives::bitcoin;

#[derive(Serialize)]
struct AppOutput {
    security: SecurityOutput,
}
#[derive(Serialize)]
struct SecurityOutput {
    blocked_addresses: Vec<bitcoin::Address>,
}

fn extract_potential_addresses(file_contents: &str) -> HashSet<String> {
    let re = Regex::new(r"\b(bc1|1|3|tb1|m|n|2)[0-9a-zA-Z]*\b").unwrap();
    let mut potential_addresses = HashSet::new();

    for cap in re.captures_iter(file_contents) {
        if let Some(match_) = cap.get(0) {
            potential_addresses.insert(match_.as_str().to_string());
        }
    }

    potential_addresses
}

fn validate_addresses(potential_addresses: HashSet<String>) -> Vec<bitcoin::Address> {
    let addresses: Vec<bitcoin::Address> = potential_addresses
        .into_iter()
        .filter_map(|addr| addr.parse::<bitcoin::Address>().ok())
        .collect();
    addresses
}
pub fn read_and_parse_addresses(file_path: impl AsRef<Path>) -> anyhow::Result<()> {
    let s = std::fs::read_to_string(file_path).context("Couldn't read file")?;
    let potential_addresses = extract_potential_addresses(&s);
    let blocked_addresses = validate_addresses(potential_addresses);

    let config = AppOutput {
        security: SecurityOutput { blocked_addresses },
    };

    let yaml = serde_yaml::to_string(&config).unwrap();
    println!("{}", yaml);
    Ok(())
}
