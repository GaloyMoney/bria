use crate::primitives::bitcoin;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;

pub fn read_and_parse_addresses(file_path: impl AsRef<Path>) -> Result<()> {
    let s = std::fs::read_to_string(file_path).context("Couldn't read file")?;
    let set: HashSet<bitcoin::Address> = s
        .split_whitespace()
        .filter_map(|word| {
            let word_without_semicolon = if word.ends_with(';') {
                word.trim_end_matches(';')
            } else {
                word
            };

            word_without_semicolon.parse::<bitcoin::Address>().ok()
        })
        .collect();

    println!("app:");
    println!("  security:");
    println!("    blocked_addresses:");
    for address in set {
        println!("    - {}", address);
    }

    Ok(())
}
