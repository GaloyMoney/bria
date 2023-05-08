mod admin_client;
mod api_client;
mod config;
mod db;
mod gen;
mod token_store;

use anyhow::Context;
use clap::{Parser, Subcommand};
use db::*;
use std::path::PathBuf;
use url::Url;

use crate::primitives::TxPriority;
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
    /// Runs the servers
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

        #[clap(env = "CRASH_REPORT_CONFIG")]
        crash_report_config: Option<bool>,
        /// Connection string for the Postgres
        #[clap(env = "PG_CON", default_value = "")]
        db_con: String,
        /// Flag to auto-bootstrap for dev
        #[clap(long)]
        dev: bool,
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
    CreateBatchGroup {
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
        #[clap(long, conflicts_with_all = &["immediate_trigger", "interval_trigger"])]
        manual_trigger: bool,
        #[clap(long, conflicts_with_all = &["manual_trigger", "interval_trigger"])]
        immediate_trigger: bool,
        #[clap(short = 'i', long, conflicts_with_all = &["manual_trigger", "immediate_trigger"])]
        interval_trigger: Option<u32>,
    },
    QueuePayout {
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
        group_name: String,
        #[clap(short, long)]
        destination: String,
        #[clap(short, long)]
        amount: u64,
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

    /// List batch groups
    ListBatchGroups {
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

    /// List signing sessions for batch
    ListSigningSessions {
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
    GenDescriptorKeys {
        #[clap(short, long, default_value = "bitcoin")]
        network: crate::primitives::bitcoin::Network,
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
            dev,
        } => {
            let config = Config::from_path(config, EnvOverride { db_con })?;
            match (
                run_cmd(&cli.bria_home, config.clone(), dev).await,
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
        Command::CreateProfile { url, api_key, name } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.create_profile(name).await?;
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
        Command::ListUtxos {
            url,
            api_key,
            wallet,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_utxos(wallet).await?;
        }
        Command::CreateBatchGroup {
            url,
            api_key,
            name,
            description,
            tx_priority,
            consolidate_deprecated_keychains,
            manual_trigger,
            immediate_trigger,
            interval_trigger,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client
                .create_batch_group(
                    name,
                    description,
                    tx_priority,
                    consolidate_deprecated_keychains,
                    manual_trigger,
                    immediate_trigger,
                    interval_trigger,
                )
                .await?;
        }
        Command::QueuePayout {
            url,
            api_key,
            wallet,
            group_name,
            destination,
            amount,
            metadata,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client
                .queue_payout(wallet, group_name, destination, amount, metadata)
                .await?;
        }
        Command::ListPayouts {
            url,
            api_key,
            wallet,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_payouts(wallet).await?;
        }
        Command::ListWallets { url, api_key } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_wallets().await?;
        }
        Command::ListBatchGroups { url, api_key } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_batch_groups().await?;
        }
        Command::ListXpubs { url, api_key } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_xpubs().await?;
        }
        Command::ListSigningSessions {
            url,
            api_key,
            batch_id,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.list_signing_sessions(batch_id).await?;
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
        Command::GenDescriptorKeys { network } => gen::gen_descriptor_keys(network)?,
    }
    Ok(())
}

fn api_client(bria_home: String, url: Option<Url>, api_key: String) -> api_client::ApiClient {
    api_client::ApiClient::new(
        bria_home,
        url.map(|url| api_client::ApiClientConfig { url })
            .unwrap_or_else(api_client::ApiClientConfig::default),
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
        blockchain,
        app,
    }: Config,
    dev: bool,
) -> anyhow::Result<()> {
    crate::tracing::init_tracer(tracing)?;
    token_store::store_daemon_pid(bria_home, std::process::id())?;
    println!("Starting server processes");
    let (send, mut receive) = tokio::sync::mpsc::channel(1);
    let mut handles = Vec::new();
    let pool = init_pool(&db).await?;

    let admin_send = send.clone();
    let admin_pool = pool.clone();
    let network = blockchain.network;
    handles.push(tokio::spawn(async move {
        let _ = admin_send.try_send(
            super::admin::run(admin_pool, admin, network)
                .await
                .context("Admin server error"),
        );
    }));
    let api_send = send.clone();
    handles.push(tokio::spawn(async move {
        let _ = api_send.try_send(
            super::api::run(pool, api, blockchain, app)
                .await
                .context("Api server error"),
        );
    }));

    if dev {
        let bria_home_string = bria_home.to_string();
        tokio::spawn(async move {
            let admin_client = admin_client::AdminApiClient::new(
                bria_home_string,
                admin_client::AdminApiClientConfig::default(),
                "".to_string(),
            );

            let mut retries = 5;
            let delay = tokio::time::Duration::from_secs(1);
            while retries > 0 {
                let dev_bootstrap_result = admin_client.dev_bootstrap().await;
                match dev_bootstrap_result {
                    Ok(_) => {
                        println!("Dev bootstrap completed successfully");
                        break;
                    }
                    Err(e) => {
                        eprintln!("Dev bootstrap failed: {:?}.\nRetrying...", e);
                        retries -= 1;
                        if retries > 0 {
                            tokio::time::sleep(delay).await;
                        } else {
                            eprintln!("Dev bootstrap failed after retries: {:?}", e);
                        }
                    }
                }
            }
        });
    }

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
        }
    }
}
