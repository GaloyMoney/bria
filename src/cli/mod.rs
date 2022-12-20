mod admin_client;
mod api_client;
mod config;
mod token_store;

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use url::Url;

use config::*;

#[derive(Parser)]
#[clap(version, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
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
        /// Connection string for the user-trades database
        #[clap(env = "PG_CON", default_value = "")]
        db_con: String,
    },
    /// Subcommand to interact with Admin API
    Admin {
        #[clap(subcommand)]
        command: AdminCommand,
        #[clap(short, long, value_parser, env = "BRIE_ADMIN_API_URL")]
        url: Option<Url>,
        #[clap(env = "BRIA_ADMIN_API_KEY", default_value = "")]
        admin_api_key: String,
    },
    /// Import an xpub
    ImportXpub {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIE_API_URL"
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
            env = "BRIE_API_URL"
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
            env = "BRIE_API_URL"
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
            env = "BRIE_API_URL"
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
            env = "BRIE_API_URL"
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
            env = "BRIE_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        name: String,
    },
    QueuePayout {
        #[clap(
            short,
            long,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIE_API_URL"
        )]
        url: Option<Url>,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
        #[clap(short, long)]
        wallet: String,
        #[clap(short, long)]
        batch_group_name: String,
        #[clap(short, long)]
        on_chain_address: String,
        #[clap(short, long)]
        amount: u64,
    },
}

#[derive(Subcommand)]
enum AdminCommand {
    Bootstrap,
    CreateAccount {
        #[clap(short, long)]
        name: String,
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
            match (run_cmd(config.clone()).await, crash_report_config) {
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
            }
        }
        Command::ImportXpub {
            url,
            api_key,
            xpub,
            name,
            derivation,
        } => {
            let client = api_client(url, api_key);
            client.import_xpub(name, xpub, derivation).await?;
        }
        Command::SetSignerConfig {
            url,
            api_key,
            xpub,
            command,
        } => {
            let client = api_client(url, api_key);
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
            let client = api_client(url, api_key);
            client.create_wallet(name, xpub).await?;
        }
        Command::WalletBalance {
            url,
            api_key,
            wallet: name,
        } => {
            let client = api_client(url, api_key);
            client.get_wallet_balance(name).await?;
        }
        Command::NewAddress {
            url,
            api_key,
            wallet,
        } => {
            let client = api_client(url, api_key);
            client.new_address(wallet).await?;
        }
        Command::CreateBatchGroup { url, api_key, name } => {
            let client = api_client(url, api_key);
            client.create_batch_group(name).await?;
        }
        Command::QueuePayout {
            url,
            api_key,
            wallet,
            batch_group_name,
            on_chain_address,
            amount,
        } => {
            let client = api_client(url, api_key);
            client
                .queue_payout(wallet, batch_group_name, on_chain_address, amount)
                .await?;
        }
    }
    Ok(())
}

fn api_client(url: Option<Url>, api_key: String) -> api_client::ApiClient {
    api_client::ApiClient::new(
        url.map(|url| api_client::ApiClientConfig { url })
            .unwrap_or_else(api_client::ApiClientConfig::default),
        api_key,
    )
}
async fn run_cmd(
    Config {
        tracing,
        db_con,
        admin,
        api,
        blockchain,
        wallets,
    }: Config,
) -> anyhow::Result<()> {
    crate::tracing::init_tracer(tracing)?;
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
            super::api::run(pool, api, blockchain, wallets)
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
    Ok(base64::encode(buffer))
}
