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
    /// Runs the configured processes
    Run {
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
    Admin {
        #[clap(subcommand)]
        command: AdminCommand,
        #[clap(short, long, action, value_parser, env = "BRIE_ADMIN_API_URL")]
        url: Option<Url>,
        #[clap(env = "BRIA_ADMIN_API_KEY", default_value = "")]
        admin_api_key: String,
    },
    ImportXpub {
        #[clap(
            short,
            long,
            action,
            value_parser,
            default_value = "http://localhost:2742",
            env = "BRIE_API_URL"
        )]
        url: Option<Url>,
        #[clap(short, long, action, value_parser)]
        xpub: String,
        #[clap(short, long, action, value_parser)]
        name: String,
        #[clap(env = "BRIA_API_KEY", default_value = "")]
        api_key: String,
    },
}

#[derive(Subcommand)]
enum AdminCommand {
    Bootstrap,
    CreateAccount {
        #[clap(short, long, action, value_parser)]
        name: String,
    },
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
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
        } => {
            let client = api_client::ApiClient::new(
                url.map(|url| api_client::ApiClientConfig { url })
                    .unwrap_or_else(api_client::ApiClientConfig::default),
                api_key,
            );
            client.import_xpub(name, xpub).await?;
        }
    }
    Ok(())
}

async fn run_cmd(
    Config {
        tracing,
        db_con,
        admin,
        api,
    }: Config,
) -> anyhow::Result<()> {
    crate::tracing::init_tracer(tracing)?;
    println!("Starting server processes");
    let (send, mut receive) = tokio::sync::mpsc::channel(1);
    let mut handles = Vec::new();
    let pool = sqlx::PgPool::connect(&db_con).await?;

    println!("Starting admin server on port {}", admin.listen_port);

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
        let _ = api_send.try_send(super::api::run(pool, api).await.context("Api server error"));
    }));
    let reason = receive.recv().await.expect("Didn't receive msg");
    for handle in handles {
        handle.abort();
    }
    reason
}
