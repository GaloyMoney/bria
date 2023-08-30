mod config;
mod server;

use crate::app::{error::*, *};

pub use config::*;
pub use server::*;

pub async fn run_dev(
    pool: sqlx::PgPool,
    config: ApiConfig,
    app_cfg: AppConfig,
    xpub: Option<(String, String)>,
    derivation_path: Option<String>,
) -> Result<(), ApplicationError> {
    use crate::{
        dev_constants,
        payout_queue::*,
        profile::Profiles,
        xpub::{BitcoindSignerConfig, SignerConfig},
    };

    let app = App::run(pool.clone(), app_cfg).await?;
    if let Some((xpub, signer_endpoint)) = xpub {
        println!("Creating dev entities");
        let profile = Profiles::new(&pool)
            .find_by_key(dev_constants::BRIA_DEV_KEY)
            .await?;
        let (_, xpubs) = app
            .create_wpkh_wallet(
                &profile,
                dev_constants::DEV_WALLET_NAME.to_string(),
                xpub,
                derivation_path,
            )
            .await?;
        app.set_signer_config(
            &profile,
            xpubs[0].to_string(),
            SignerConfig::Bitcoind(BitcoindSignerConfig {
                endpoint: signer_endpoint,
                rpc_user: dev_constants::DEFAULT_BITCOIND_RPC_USER.to_string(),
                rpc_password: dev_constants::DEFAULT_BITCOIND_RPC_PASSWORD.to_string(),
            }),
        )
        .await?;
        app.create_payout_queue(
            &profile,
            dev_constants::DEV_QUEUE_NAME.to_string(),
            None,
            Some(PayoutQueueConfig {
                trigger: PayoutQueueTrigger::Interval {
                    seconds: std::time::Duration::from_secs(5),
                },
                ..PayoutQueueConfig::default()
            }),
        )
        .await?;
    }
    server::start(config, app).await?;
    Ok(())
}

pub async fn run(
    pool: sqlx::PgPool,
    config: ApiConfig,
    app_cfg: AppConfig,
) -> Result<(), ApplicationError> {
    let app = App::run(pool, app_cfg).await?;
    server::start(config, app).await?;
    Ok(())
}
