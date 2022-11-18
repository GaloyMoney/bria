mod config;

use anyhow::Context;
use clap::{Parser, Subcommand};
use std::{collections::HashMap, path::PathBuf};

use config::*;

#[derive(Parser)]
#[clap(version, long_about = None)]
struct Cli {
    /// Sets a custom config file
    #[clap(
        short,
        long,
        env = "BRIA_CONFIG",
        default_value = "bria.yml",
        value_name = "FILE"
    )]
    config: PathBuf,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Runs the configured processes
    Run {
        #[clap(env = "CRASH_REPORT_CONFIG")]
        crash_report_config: Option<bool>,
        /// Connection string for the user-trades database
        #[clap(env = "PG_CON", default_value = "")]
        db_con: String,
    },
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run {
            crash_report_config,
            db_con,
        } => {
            let config = Config::from_path(cli.config, EnvOverride { db_con })?;
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
    }
    Ok(())
}

async fn run_cmd(
    Config {
        tracing,
        db_con,
        admin,
    }: Config,
) -> anyhow::Result<()> {
    crate::tracing::init_tracer(tracing)?;
    println!("Starting server processes");
    let (send, mut receive) = tokio::sync::mpsc::channel(1);
    let mut handles = Vec::new();
    let pool = sqlx::PgPool::connect(&db_con).await?;

    println!("Starting admin server on port {}", admin.listen_port);

    let admin_send = send.clone();
    handles.push(tokio::spawn(async move {
        let _ = admin_send.try_send(
            super::admin::run(pool, admin)
                .await
                .context("Admin server error"),
        );
    }));
    let reason = receive.recv().await.expect("Didn't receive msg");
    for handle in handles {
        handle.abort();
    }
    reason
}
