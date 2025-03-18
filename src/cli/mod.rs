mod address_extractor;
mod admin_client;
mod api_client;
mod config;
mod db;
mod gen;

use anyhow::Context;
use clap::{Parser, Subcommand};
use db::*;
use std::path::PathBuf;
use url::Url;

use crate::{
    dev_constants,
    primitives::{bitcoin, TxPriority},
    token_store,
};
use config::*;

#[derive(Parser)]
#[clap(version, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Command,

    /// Directory for storing tokens + pid file
    #[clap(
        short,
        long,
        env = "BRIA_HOME",
        default_value = ".bria",
        value_name = "DIRECTORY"
    )]
    bria_home: String,
}

#[derive(Subcommand)]
enum Command {
    /// Subcommand for running the server
    Daemon {
        /// Sets a custom config file
        #[clap(
            short,
            long,
            env = "BRIA_CONFIG",
            default_value = "bria.yml",
            value_name = "FILE"
        )]
        config: PathBuf,
        /// Connection string for the Postgres
        #[clap(env = "PG_CON")]
        db_con: String,
        #[clap(env = "CRASH_REPORT_CONFIG")]
        crash_report_config: Option<bool>,
        #[clap(subcommand)]
        command: DaemonCommand,
    },
    /// Subcommand to interact with Admin API
    Admin {
        #[clap(subcommand)]
        command: AdminCommand,
        #[clap(short, long, value_parser, env = "BRIA_ADMIN_API_URL")]
        url: Option<Url>,
        #[clap(env = "BRIA_ADMIN_API_KEY", default_value = "")]
        admin_api_key: String,
    },
    /// Subcommand for various utilities
    Utils {
        #[clap(subcommand)]
        command: UtilsCommand,
    },
    /// Create a new profile
    CreateProfile {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        name: String,
        /// Allowed payout addresses for the spending policy
        #[clap(short, long)]
        addresses: Option<Vec<String>>,
        /// The max payout amount in Satoshi
        #[clap(short, long)]
        max_payout: Option<u64>,
    },
    /// Update a profile
    UpdateProfile {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        /// The id to update
        #[clap(short, long)]
        id: String,
        /// Allowed payout addresses for the spending policy
        #[clap(short, long)]
        addresses: Option<Vec<String>>,
        /// The max payout amount in Satoshi
        #[clap(short, long)]
        max_payout: Option<u64>,
    },
    /// List all profiles
    ListProfiles {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
    },
    /// Generate a new Api Key for the given profile name
    GenApiKey {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        profile: String,
    },
    /// Import an xpub
    ImportXpub {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        /// The name to be associated with the key
        #[clap(short, long)]
        name: String,
        /// The base58 encoded extended public key
        #[clap(short, long)]
        xpub: String,
        /// The derivation from the parent key (eg. m/84'/0'/0')
        #[clap(short, long)]
        derivation: Option<String>,
    },
    /// List Xpubs
    ListXpubs {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
    },
    SetSignerConfig {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        xpub: String,
        #[clap(subcommand)]
        command: SetSignerConfigCommand,
    },
    /// Submit a signed psbt
    SubmitSignedPsbt {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        batch_id: String,
        #[clap(short, long)]
        xpub_ref: String,
        #[clap(short, long)]
        signed_psbt: String,
    },
    /// Create a wallet from imported xpubs
    CreateWallet {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        name: String,
        #[clap(subcommand)]
        command: CreateWalletCommand,
    },
    /// Report the balance of a wallet (as reflected in the ledger)
    WalletBalance {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        wallet: String,
    },

    AccountBalance {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
    },

    /// Get a new address for a wallet
    NewAddress {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        wallet: String,
        #[clap(short, long)]
        external_id: Option<String>,
        #[clap(short, long, value_parser = parse_json)]
        metadata: Option<serde_json::Value>,
    },
    /// Update address information
    UpdateAddress {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        /// The address to update
        #[clap(short, long)]
        address: String,
        /// The new external id
        #[clap(short, long)]
        external_id: Option<String>,
        /// The new metadata id
        #[clap(short, long, value_parser = parse_json)]
        metadata: Option<serde_json::Value>,
    },
    /// List external addresses up to a given path index
    ListAddresses {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        wallet: String,
    },
    /// Find address by external id or address
    GetAddress {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short = 'a', long, group = "identifier")]
        address: Option<String>,
        #[clap(short = 'e', long, group = "identifier")]
        external_id: Option<String>,
    },
    /// List Unspent Transaction Outputs of a wallet
    ListUtxos {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        wallet: String,
    },
    /// Create a Payuot Queue
    CreatePayoutQueue {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        name: String,
        #[clap(short, long)]
        description: Option<String>,
        #[clap(short = 'p', long, default_value = "next-block")]
        tx_priority: TxPriority,
        #[clap(short = 'c', long = "consolidate", default_value = "true")]
        consolidate_deprecated_keychains: bool,
        #[clap(short = 'i', long = "interval-trigger")]
        interval_trigger: Option<u32>,
        #[clap(short = 'm', long = "manual")]
        manual_trigger: Option<bool>,
        #[clap(long = "cpfp-after-mins")]
        cpfp_payouts_after_mins: Option<u32>,
        #[clap(long = "cpfp-after-blocks")]
        cpfp_payouts_after_blocks: Option<u32>,
        #[clap(long)]
        min_change: Option<u64>,
    },
    /// Trigger Payout Queue
    TriggerPayoutQueue {
        #[clap(short, long, value_parser, default_value = "http://localhost:2742")]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        name: String,
    },
    EstimatePayoutFee {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        wallet: String,
        #[clap(short, long)]
        queue_name: String,
        #[clap(short, long)]
        destination: String,
        #[clap(short, long)]
        amount: u64,
    },
    SubmitPayout {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        wallet: String,
        #[clap(short, long)]
        queue_name: String,
        #[clap(short, long)]
        destination: String,
        #[clap(short, long)]
        amount: u64,
        #[clap(short, long)]
        external_id: Option<String>,
        #[clap(short, long, value_parser = parse_json)]
        metadata: Option<serde_json::Value>,
    },
    /// List pending Payouts
    ListPayouts {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        wallet: String,
        #[clap(short, long)]
        page: Option<u64>,
        #[clap(short = 's', long = "page-size")]
        page_size: Option<u64>,
    },
    /// Find Payout By external id or payout_id
    GetPayout {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short = 'i', long, group = "identifier")]
        id: Option<String>,
        #[clap(short = 'e', long, group = "identifier")]
        external_id: Option<String>,
    },
    CancelPayout {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short = 'i', long)]
        id: String,
    },
    /// List Wallets
    ListWallets {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
    },

    /// List Payout Queue
    ListPayoutQueues {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
    },
    /// Update Payout Queue
    UpdatePayoutQueue {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        /// The id to update
        #[clap(short, long)]
        id: String,
        ///  The new description
        #[clap(short, long)]
        description: Option<String>,
        #[clap(short = 'p', long, default_value = "next-block")]
        tx_priority: Option<TxPriority>,
        #[clap(short = 'c', long = "consolidate", default_value = "true")]
        consolidate_deprecated_keychains: Option<bool>,
        #[clap(long = "interval-trigger")]
        interval_trigger: Option<u32>,
        #[clap(long = "cpfp-after-mins")]
        cpfp_payouts_after_mins: Option<u32>,
        #[clap(long = "cpfp-after-blocks")]
        cpfp_payouts_after_blocks: Option<u32>,
        #[clap(long)]
        min_change: Option<u64>,
    },
    /// Get Batch details
    GetBatch {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        batch_id: String,
    },
    CancelBatch {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        batch_id: String,
    },
    /// Watch or fetch events
    WatchEvents {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIA_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        /// If set, only fetch the next event and exit
        #[clap(short, long)]
        one_shot: bool,
        /// The sequence number after which to stream
        #[clap(short, long)]
        after: Option<u64>,
        /// Include augmented information in events
        #[clap(long, default_value = "false")]
        augment: bool,
    },
}

#[derive(Subcommand)]
enum UtilsCommand {
    /// generate a seed private key and derived descriptors
    GenDescriptorKeys {
        #[clap(short, long, default_value = "bitcoin")]
        network: bitcoin::Network,
    },
    /// generate a hex encoded 32 byte random key
    GenSignerEncryptionKey {},
    /// extract addresses
    ExtractAddresses { path: PathBuf },
    /// generate a new encryption key, encrypted deprecated key and nonce
    RotateSignerEncryptionKey {
        #[clap(short, long)]
        old_key: String,
    },
}

#[derive(Subcommand)]
enum DaemonCommand {
    Run {
        #[clap(env = "SIGNER_ENCRYPTION_KEY")]
        signer_encryption_key: String,
    },
    Dev {
        #[clap(short = 'x', long = "xpub")]
        /// The base58 encoded extended public key
        xpub: Option<String>,
        /// The derivation from the parent key (eg. m/84'/0'/0')
        #[clap(short = 'd', long = "derivation")]
        derivation: Option<String>,
        #[clap(
            env = "BITCOIND_SIGNER_ENDPOINT",
            default_value = "https://localhost:18543"
        )]
        bitcoind_signer_endpoint: String,
    },
}

#[derive(Subcommand)]
enum AdminCommand {
    Bootstrap,
    CreateAccount {
        #[clap(short, long)]
        name: String,
    },
    ListAccounts {},
}

#[derive(Subcommand)]
enum CreateWalletCommand {
    /// Initialize the wallet via wpkh
    Wpkh {
        /// The xpub-ref or xpub to use
        #[clap(short, long)]
        xpub: String,
        /// If an xpub is being imported, the derivation path to use
        #[clap(short, long)]
        derivation: Option<String>,
    },
    /// Initialize the wallet via descriptors
    Descriptors {
        /// The descriptor for external addresses
        #[clap(short, long)]
        descriptor: String,
        /// The descriptor for internal addresses
        #[clap(short, long)]
        change_descriptor: String,
    },
    SortedMultisig {
        #[clap(short, long, num_args(2..=15) )]
        xpub: Vec<String>,
        #[clap(short, long)]
        threshold: u32,
    },
}

#[derive(Subcommand)]
enum SetSignerConfigCommand {
    Lnd {
        #[clap(short, long)]
        endpoint: String,
        #[clap(short, long)]
        macaroon_file: PathBuf,
        #[clap(short, long)]
        cert_file: PathBuf,
    },
    Bitcoind {
        #[clap(short, long)]
        endpoint: String,
        #[clap(short = 'u', long)]
        rpc_user: String,
        #[clap(short = 'p', long)]
        rpc_password: String,
    },
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Daemon {
            config,
            crash_report_config,
            db_con,
            command,
        } => {
            let (dev, dev_xpub, dev_derivation, signer_encryption_key) = match command {
                DaemonCommand::Dev {
                    xpub,
                    derivation,
                    bitcoind_signer_endpoint,
                } => (
                    true,
                    xpub.map(|xpub| (xpub, bitcoind_signer_endpoint)),
                    derivation,
                    dev_constants::DEV_SIGNER_ENCRYPTION_KEY.to_string(),
                ),
                DaemonCommand::Run {
                    signer_encryption_key,
                } => (false, None, None, signer_encryption_key),
            };
            let config = Config::from_path(
                config,
                EnvOverride {
                    db_con,
                    signer_encryption_key,
                },
            )?;
            if dev && config.app.blockchain.network == bitcoin::Network::Bitcoin {
                return Err(anyhow::anyhow!("Dev mode is not allowed for mainnet"));
            }
            match (
                run_cmd(
                    &cli.bria_home,
                    config.clone(),
                    dev,
                    dev_xpub,
                    dev_derivation,
                )
                .await,
                crash_report_config,
            ) {
                (Err(e), Some(true)) => {
                    println!("Bria was started with the following config:");
                    println!("{}", serde_yaml::to_string(&config).unwrap());
                    return Err(e);
                }
                (Err(e), _) => return Err(e),
                _ => (),
            }
        }
        Command::Utils { command } => match command {
            UtilsCommand::GenDescriptorKeys { network } => gen::gen_descriptor_keys(network)?,
            UtilsCommand::GenSignerEncryptionKey {} => gen::gen_signer_encryption_key()?,
            UtilsCommand::ExtractAddresses { path } => {
                address_extractor::read_and_parse_addresses(path)?
            }
            UtilsCommand::RotateSignerEncryptionKey { old_key } => {
                gen::rotate_signer_encryption_key(old_key)?
            }
        },
        Command::Admin {
            command,
            url,
            admin_api_key,
        } => {
            let client = admin_client::AdminApiClient::new(
                cli.bria_home,
                url.map(|url| admin_client::AdminApiClientConfig { url })
                    .unwrap_or_else(admin_client::AdminApiClientConfig::default),
                admin_api_key,
            );
            match command {
                AdminCommand::Bootstrap => {
                    client.bootstrap().await?;
                }
                AdminCommand::CreateAccount { name } => {
                    client.account_create(name).await?;
                }
                AdminCommand::ListAccounts {} => {
                    client.list_accounts().await?;
                }
            }
        }
        Command::CreateProfile {
            url,
            api_key,
            name,
            addresses,
            max_payout,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.create_profile(name, addresses, max_payout).await?;
        }
        Command::UpdateProfile {
            url,
            api_key,
            id,
            addresses,
            max_payout,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.update_profile(id, addresses, max_payout).await?;
        }
        Command::ListProfiles { url, api_key } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_profiles().await?;
        }
        Command::GenApiKey {
            url,
            api_key,
            profile,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.create_profile_api_key(profile).await?;
        }
        Command::ImportXpub {
            url,
            api_key,
            xpub,
            name,
            derivation,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.import_xpub(name, xpub, derivation).await?;
        }
        Command::SetSignerConfig {
            url,
            api_key,
            xpub,
            command,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.set_signer_config(xpub, command).await?;
        }
        Command::SubmitSignedPsbt {
            url,
            api_key,
            batch_id,
            xpub_ref,
            signed_psbt,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client
                .submit_signed_psbt(batch_id, xpub_ref, signed_psbt)
                .await?;
        }
        Command::CreateWallet {
            url,
            api_key,
            name,
            command,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.create_wallet(name, command).await?;
        }
        Command::WalletBalance {
            url,
            api_key,
            wallet: name,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.get_wallet_balance_summary(name).await?;
        }
        Command::AccountBalance { url, api_key } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.get_account_balance_summary().await?;
        }
        Command::NewAddress {
            url,
            api_key,
            wallet,
            external_id,
            metadata,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.new_address(wallet, external_id, metadata).await?;
        }
        Command::UpdateAddress {
            url,
            api_key,
            address,
            external_id,
            metadata,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client
                .update_address(address, external_id, metadata)
                .await?;
        }
        Command::ListAddresses {
            url,
            api_key,
            wallet,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_addresses(wallet).await?;
        }
        Command::GetAddress {
            url,
            api_key,
            address,
            external_id,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.get_address(address, external_id).await?;
        }
        Command::ListUtxos {
            url,
            api_key,
            wallet,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_utxos(wallet).await?;
        }
        Command::CreatePayoutQueue {
            url,
            api_key,
            name,
            description,
            tx_priority,
            consolidate_deprecated_keychains,
            interval_trigger,
            manual_trigger,
            cpfp_payouts_after_mins,
            cpfp_payouts_after_blocks,
            min_change,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client
                .create_payout_queue(
                    name,
                    description,
                    tx_priority,
                    consolidate_deprecated_keychains,
                    interval_trigger,
                    manual_trigger,
                    cpfp_payouts_after_mins,
                    cpfp_payouts_after_blocks,
                    min_change,
                )
                .await?;
        }
        Command::TriggerPayoutQueue { url, api_key, name } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.trigger_payout_queue(name).await?;
        }
        Command::EstimatePayoutFee {
            url,
            api_key,
            wallet,
            queue_name: group_name,
            destination,
            amount,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client
                .estimate_payout_fee(wallet, group_name, destination, amount)
                .await?;
        }
        Command::SubmitPayout {
            url,
            api_key,
            wallet,
            queue_name: group_name,
            destination,
            amount,
            external_id,
            metadata,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client
                .submit_payout(
                    wallet,
                    group_name,
                    destination,
                    amount,
                    external_id,
                    metadata,
                )
                .await?;
        }
        Command::ListPayouts {
            url,
            api_key,
            wallet,
            page,
            page_size,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_payouts(wallet, page, page_size).await?;
        }
        Command::GetPayout {
            url,
            api_key,
            id,
            external_id,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.get_payout(id, external_id).await?;
        }
        Command::CancelPayout { url, api_key, id } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.cancel_payout(id).await?;
        }
        Command::ListWallets { url, api_key } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_wallets().await?;
        }
        Command::ListPayoutQueues { url, api_key } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_payout_queues().await?;
        }
        Command::UpdatePayoutQueue {
            url,
            api_key,
            id,
            description,
            tx_priority,
            consolidate_deprecated_keychains,
            interval_trigger,
            cpfp_payouts_after_mins,
            cpfp_payouts_after_blocks,
            min_change,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client
                .update_payout_queue(
                    id,
                    description,
                    tx_priority,
                    consolidate_deprecated_keychains,
                    interval_trigger,
                    cpfp_payouts_after_mins,
                    cpfp_payouts_after_blocks,
                    min_change,
                )
                .await?;
        }
        Command::ListXpubs { url, api_key } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_xpubs().await?;
        }
        Command::GetBatch {
            url,
            api_key,
            batch_id,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.get_batch(batch_id).await?;
        }
        Command::CancelBatch {
            url,
            api_key,
            batch_id,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.cancel_batch(batch_id).await?;
        }
        Command::WatchEvents {
            url,
            api_key,
            one_shot,
            after,
            augment,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.watch_events(one_shot, after, augment).await?;
        }
    }
    Ok(())
}

fn api_client(bria_home: String, url: Option<Url>, api_key: String) -> api_client::ApiClient {
    api_client::ApiClient::new(
        bria_home,
        url.map(|url| api_client::ApiClientConfig { url })
            .unwrap_or_default(),
        api_key,
    )
}
async fn run_cmd(
    bria_home: &str,
    Config {
        tracing,
        db,
        admin,
        api,
        app,
    }: Config,
    dev: bool,
    dev_xpub: Option<(String, String)>,
    dev_derivation: Option<String>,
) -> anyhow::Result<()> {
    crate::tracing::init_tracer(tracing)?;
    token_store::store_daemon_pid(bria_home, std::process::id())?;
    let bria_home = bria_home.to_string();
    println!("Starting server processes");
    let (send, mut receive) = tokio::sync::mpsc::channel(1);
    let mut handles = Vec::new();
    let pool = init_pool(&db).await?;

    let admin_send = send.clone();
    let admin_pool = pool.clone();
    let network = app.blockchain.network;
    handles.push(tokio::spawn(async move {
        let _ = admin_send.try_send(if dev {
            super::admin::run_dev(admin_pool, admin, network, bria_home)
                .await
                .context("Admin server error")
        } else {
            super::admin::run(admin_pool, admin, network)
                .await
                .context("Admin server error")
        });
    }));
    let api_send = send.clone();
    handles.push(tokio::spawn(async move {
        let _ = api_send.try_send(if dev {
            super::api::run_dev(pool, api, app, dev_xpub, dev_derivation)
                .await
                .context("Api server error")
        } else {
            super::api::run(pool, api, app)
                .await
                .context("Api server error")
        });
    }));

    let reason = receive.recv().await.expect("Didn't receive msg");
    for handle in handles {
        handle.abort();
    }

    reason
}

fn read_to_base64(path: PathBuf) -> anyhow::Result<String> {
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Read;
    let f = File::open(path)?;
    let mut reader = BufReader::new(f);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;
    use base64::{engine::general_purpose, Engine};
    Ok(general_purpose::STANDARD.encode(buffer))
}

fn parse_json(src: &str) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::from_str(src)?)
}

impl TryFrom<SetSignerConfigCommand> for crate::api::proto::set_signer_config_request::Config {
    type Error = anyhow::Error;

    fn try_from(cmd: SetSignerConfigCommand) -> Result<Self, Self::Error> {
        use crate::api::proto::set_signer_config_request::Config;
        let ret = match cmd {
            SetSignerConfigCommand::Lnd {
                endpoint,
                macaroon_file,
                cert_file,
            } => {
                let macaroon_base64 = read_to_base64(macaroon_file)?;
                let cert_base64 = read_to_base64(cert_file)?;
                Config::Lnd(crate::api::proto::LndSignerConfig {
                    endpoint,
                    macaroon_base64,
                    cert_base64,
                })
            }
            SetSignerConfigCommand::Bitcoind {
                endpoint,
                rpc_user,
                rpc_password,
            } => Config::Bitcoind(crate::api::proto::BitcoindSignerConfig {
                endpoint,
                rpc_user,
                rpc_password,
            }),
        };
        Ok(ret)
    }
}

impl From<CreateWalletCommand> for crate::api::proto::keychain_config::Config {
    fn from(command: CreateWalletCommand) -> Self {
        use crate::api::proto::keychain_config::*;
        match command {
            CreateWalletCommand::Wpkh { xpub, derivation } => Config::Wpkh(Wpkh {
                xpub,
                derivation_path: derivation,
            }),
            CreateWalletCommand::Descriptors {
                descriptor,
                change_descriptor,
            } => Config::Descriptors(Descriptors {
                external: descriptor,
                internal: change_descriptor,
            }),
            CreateWalletCommand::SortedMultisig { xpub, threshold } => {
                Config::SortedMultisig(SortedMultisig {
                    xpubs: xpub,
                    threshold,
                })
            }
        }
    }
}
