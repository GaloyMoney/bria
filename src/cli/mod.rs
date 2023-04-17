mod admin_client;
mod api_client;
mod config;
mod token_store;

use anyhow::Context;
use clap::{Parser, Subcommand};
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
        xpub: Vec<String>,
        #[clap(short, long)]
        name: String,
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
        metadata: serde_json::Value,
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
enum SetSignerConfigCommand {
    Lnd {
        #[clap(short, long)]
        endpoint: String,
        #[clap(short, long)]
        macaroon_file: PathBuf,
        #[clap(short, long)]
        cert_file: PathBuf,
    },
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Daemon {
            config,
            crash_report_config,
            db_con,
        } => {
            let config = Config::from_path(config, EnvOverride { db_con })?;
            match (
                run_cmd(&cli.bria_home, config.clone()).await,
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
            match command {
                SetSignerConfigCommand::Lnd {
                    endpoint,
                    macaroon_file,
                    cert_file,
                } => {
                    let macaroon_base64 = read_to_base64(macaroon_file)?;
                    let cert_base64 = read_to_base64(cert_file)?;
                    client
                        .set_signer_config(
                            xpub,
                            crate::api::proto::LndSignerConfig {
                                endpoint,
                                macaroon_base64,
                                cert_base64,
                            },
                        )
                        .await?;
                }
            };
        }
        Command::CreateWallet {
            url,
            api_key,
            xpub,
            name,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.create_wallet(name, xpub).await?;
        }
        Command::WalletBalance {
            url,
            api_key,
            wallet: name,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.get_wallet_balance_summary(name).await?;
        }
        Command::NewAddress {
            url,
            api_key,
            wallet,
        } => {
            let client = api_client(cli.bria_home, url, api_key);
            client.new_address(wallet).await?;
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
        db_con,
        migrate_on_start,
        admin,
        api,
        blockchain,
        wallets,
    }: Config,
) -> anyhow::Result<()> {
    crate::tracing::init_tracer(tracing)?;
    token_store::store_daemon_pid(bria_home, std::process::id())?;
    println!("Starting server processes");
    let (send, mut receive) = tokio::sync::mpsc::channel(1);
    let mut handles = Vec::new();
    let pool = sqlx::PgPool::connect(&db_con).await?;

    let admin_send = send.clone();
    let admin_pool = pool.clone();
    handles.push(tokio::spawn(async move {
        let _ = admin_send.try_send(
            super::admin::run(admin_pool, admin)
                .await
                .context("Admin server error"),
        );
    }));
    let api_send = send.clone();
    handles.push(tokio::spawn(async move {
        let _ = api_send.try_send(
            super::api::run(pool, api, migrate_on_start, blockchain, wallets)
                .await
                .context("Api server error"),
        );
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

fn parse_json(src: &str) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::from_str(src)
}
