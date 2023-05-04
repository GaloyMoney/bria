use bdk::{
    bitcoin::{secp256k1::Secp256k1, util::bip32, Network},
    keys::{DerivableKey, DescriptorKey, GeneratableKey, GeneratedKey},
    miniscript::Segwitv0,
};

use std::str::FromStr;

pub fn gen_descriptor_wallet(network: Network) -> anyhow::Result<()> {
    let root_key: GeneratedKey<bip32::ExtendedPrivKey, Segwitv0> =
        bip32::ExtendedPrivKey::generate(())?;
    let root_key = root_key.into_extended_key()?.into_xprv(network).unwrap();
    println!("ROOT KEY\n{}", root_key);
    let path = bip32::DerivationPath::from_str("m/84'/0'/0'")?;
    let secp = Secp256k1::new();
    let external_key: DescriptorKey<Segwitv0> = root_key
        .derive_priv(&secp, &path)
        .unwrap()
        .into_descriptor_key(
            Some((root_key.fingerprint(&secp), path.clone())),
            bip32::DerivationPath::from_str("m/0").unwrap(),
        )?;
    let internal_key: DescriptorKey<Segwitv0> = root_key
        .derive_priv(&secp, &path)
        .unwrap()
        .into_descriptor_key(
            Some((root_key.fingerprint(&secp), path.clone())),
            bip32::DerivationPath::from_str("m/1").unwrap(),
        )?;
    let external_descriptor = bdk::descriptor!(wpkh(external_key)).unwrap();
    let internal_descriptor = bdk::descriptor!(wpkh(internal_key)).unwrap();
    let bitcoind_json = serde_json::json!([{
        "desc": external_descriptor.0.to_string_with_secret(&external_descriptor.1),
        "active": true,
        "timestamp": "now",
    },
    {
        "desc": internal_descriptor.0.to_string_with_secret(&internal_descriptor.1),
        "active": true,
        "internal": true,
        "timestamp": "now",
    }
    ]);
    println!("BITCOIND\n{}", bitcoind_json);
    println!("EXTERNAL\n{}", external_descriptor.0);
    println!("INTERNAL\n{}", internal_descriptor.0);
    let wallet = bdk::Wallet::new(
        external_descriptor.0,
        Some(internal_descriptor.0),
        network,
        bdk::database::MemoryDatabase::default(),
    )?;
    println!("{}", wallet.get_address(bdk::wallet::AddressIndex::New)?);
    println!("{}", wallet.get_address(bdk::wallet::AddressIndex::New)?);
    println!("{}", wallet.get_address(bdk::wallet::AddressIndex::New)?);

    Ok(())
}
