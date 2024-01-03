mod batch_broadcasting;
mod batch_signing;
mod batch_wallet_accounting;
mod config;
mod executor;
mod populate_outbox;
mod sync_wallet;

pub mod error;
pub mod process_payout_queue;

pub use config::*;

use sqlxmq::{job, CurrentJob, JobBuilder, JobRegistry, JobRunnerHandle};
use tracing::instrument;
use uuid::{uuid, Uuid};

use crate::{
    account::*, address::Addresses, app::BlockchainConfig, batch::*, fees::FeesClient,
    ledger::Ledger, outbox::*, payout::*, payout_queue::*, primitives::*, signing_session::*,
    utxo::Utxos, wallet::*, xpub::*,
};
use batch_broadcasting::BatchBroadcastingData;
use batch_signing::BatchSigningData;
use batch_wallet_accounting::BatchWalletAccountingData;
use error::JobError;
pub use executor::JobExecutionError;
use executor::JobExecutor;
use populate_outbox::PopulateOutboxData;
use process_payout_queue::ProcessPayoutQueueData;
use sync_wallet::SyncWalletData;

const SYNC_ALL_WALLETS_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
const PROCESS_ALL_PAYOUT_QUEUES_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");
const RESPAWN_ALL_OUTBOX_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000003");

#[allow(clippy::too_many_arguments)]
pub async fn start_job_runner(
    pool: &sqlx::PgPool,
    outbox: Outbox,
    wallets: Wallets,
    xpubs: XPubs,
    payout_queues: PayoutQueues,
    batches: Batches,
    signing_sessions: SigningSessions,
    payouts: Payouts,
    ledger: Ledger,
    utxos: Utxos,
    addresses: Addresses,
    config: JobsConfig,
    blockchain_cfg: BlockchainConfig,
    signer_encryption_config: SignerEncryptionConfig,
    fees_client: FeesClient,
) -> Result<JobRunnerHandle, JobError> {
    let mut registry = JobRegistry::new(&[
        sync_all_wallets,
        sync_wallet,
        process_all_payout_queues,
        schedule_process_payout_queue,
        process_payout_queue,
        batch_wallet_accounting,
        batch_signing,
        batch_broadcasting,
        respawn_all_outbox_handlers,
        populate_outbox,
    ]);
    registry.set_context(config);
    registry.set_context(blockchain_cfg);
    registry.set_context(outbox);
    registry.set_context(wallets);
    registry.set_context(xpubs);
    registry.set_context(payout_queues);
    registry.set_context(batches);
    registry.set_context(signing_sessions);
    registry.set_context(payouts);
    registry.set_context(ledger);
    registry.set_context(utxos);
    registry.set_context(addresses);
    registry.set_context(signer_encryption_config);
    registry.set_context(fees_client);

    Ok(registry.runner(pool).set_keep_alive(false).run().await?)
}

#[job(name = "sync_all_wallets")]
async fn sync_all_wallets(
    mut current_job: CurrentJob,
    wallets: Wallets,
    JobsConfig {
        sync_all_wallets_delay: delay,
        ..
    }: JobsConfig,
) -> Result<(), JobError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|_| async move {
            for (account_id, wallet_id) in wallets.all_ids().await? {
                let _ = spawn_sync_wallet(&pool, SyncWalletData::new(account_id, wallet_id)).await;
            }
            Ok::<(), JobError>(())
        })
        .await?;
    spawn_sync_all_wallets(current_job.pool(), delay).await?;
    Ok(())
}

#[job(name = "process_all_payout_queues")]
async fn process_all_payout_queues(
    mut current_job: CurrentJob,
    payout_queues: PayoutQueues,
    JobsConfig {
        process_all_payout_queues_delay: delay,
        ..
    }: JobsConfig,
) -> Result<(), JobError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|_| async move {
            for group in payout_queues.all().await? {
                if let Some(delay) = group.spawn_in() {
                    let _ = spawn_schedule_process_payout_queue(
                        &pool,
                        (group.account_id, group.id),
                        delay
                            .checked_sub(std::time::Duration::from_secs(1))
                            .unwrap_or_default(),
                    )
                    .await;
                }
            }
            Ok::<(), JobError>(())
        })
        .await?;
    spawn_process_all_payout_queues(current_job.pool(), delay).await?;
    Ok(())
}

#[job(name = "populate_outbox")]
async fn populate_outbox(
    mut current_job: CurrentJob,
    outbox: Outbox,
    ledger: Ledger,
) -> Result<(), JobError> {
    JobExecutor::builder(&mut current_job)
        .max_retry_delay(std::time::Duration::from_secs(20))
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: PopulateOutboxData = data.expect("no PopulateOutboxData available");
            let data = populate_outbox::execute(data, outbox, ledger).await?;
            Ok::<_, JobError>(data)
        })
        .await?;
    Ok(())
}

#[job(name = "respawn_all_outbox_handlers")]
async fn respawn_all_outbox_handlers(
    mut current_job: CurrentJob,
    JobsConfig {
        respawn_all_outbox_handlers_delay: delay,
        ..
    }: JobsConfig,
) -> Result<(), JobError> {
    let pool = current_job.pool().clone();
    let accounts = Accounts::new(&pool);
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|_| async move {
            for account in accounts.list().await? {
                let _ = spawn_outbox_handler(&pool, account).await;
            }
            Ok::<(), JobError>(())
        })
        .await?;
    spawn_respawn_all_outbox_handlers(current_job.pool(), delay).await?;
    Ok(())
}

#[job(name = "sync_wallet")]
#[allow(clippy::too_many_arguments)]
async fn sync_wallet(
    mut current_job: CurrentJob,
    wallets: Wallets,
    blockchain_cfg: BlockchainConfig,
    addresses: Addresses,
    utxos: Utxos,
    ledger: Ledger,
    batches: Batches,
    fees_client: FeesClient,
) -> Result<(), JobError> {
    let pool = current_job.pool().clone();
    let mut has_more = false;
    let more_ref = &mut has_more;
    let data = JobExecutor::builder(&mut current_job)
        .max_retry_delay(std::time::Duration::from_secs(60))
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: SyncWalletData = data.expect("no SyncWalletData available");
            let (more, data) = sync_wallet::execute(
                pool,
                wallets,
                blockchain_cfg,
                utxos,
                addresses,
                ledger,
                batches,
                data,
                fees_client,
            )
            .await?;
            *more_ref = more;
            Ok::<_, JobError>(data)
        })
        .await?;
    if has_more {
        spawn_sync_wallet(current_job.pool(), data).await?;
    }
    Ok(())
}

pub async fn spawn_process_payout_queue(
    pool: &sqlx::PgPool,
    data: impl Into<ProcessPayoutQueueData>,
) -> Result<ProcessPayoutQueueData, JobError> {
    let data = data.into();
    onto_account_main_channel(
        pool,
        data.account_id,
        Uuid::new_v4(),
        "process_payout_queue",
        data,
    )
    .await
}

#[job(name = "schedule_process_payout_queue")]
async fn schedule_process_payout_queue(mut current_job: CurrentJob) -> Result<(), JobError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let mut data: ProcessPayoutQueueData = data.expect("no SyncWalletData available");
            data.tracing_data = crate::tracing::extract_tracing_data();
            spawn_process_payout_queue(&pool, data).await
        })
        .await?;
    Ok(())
}

#[job(name = "process_payout_queue")]
async fn process_payout_queue(
    mut current_job: CurrentJob,
    payouts: Payouts,
    wallets: Wallets,
    utxos: Utxos,
    payout_queues: PayoutQueues,
    batches: Batches,
    fees_client: FeesClient,
) -> Result<(), JobError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .initial_retry_delay(std::time::Duration::from_secs(2))
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: ProcessPayoutQueueData = data.expect("no ProcessPayoutQueueData available");
            let (data, res) = process_payout_queue::execute(
                pool,
                payouts,
                wallets,
                payout_queues,
                batches,
                utxos,
                data,
                fees_client,
            )
            .await?;
            if let Some((mut tx, wallet_ids)) = res {
                for id in wallet_ids {
                    spawn_batch_wallet_accounting(&mut tx, (&data, id)).await?;
                }
                spawn_batch_signing(tx, &data).await?;
            }

            Ok::<_, JobError>(data)
        })
        .await?;
    Ok(())
}

#[job(
    name = "batch_wallet_accounting",
    channel_name = "wallet_accounting",
    retries = 20,
    ordered = true
)]
async fn batch_wallet_accounting(
    mut current_job: CurrentJob,
    blockchain_cfg: BlockchainConfig,
    ledger: Ledger,
    wallets: Wallets,
    utxos: Utxos,
    batches: Batches,
    payouts: Payouts,
) -> Result<(), JobError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: BatchWalletAccountingData =
                data.expect("no BatchWalletAccountingData available");
            let res = batch_wallet_accounting::execute(
                data.clone(),
                blockchain_cfg,
                ledger,
                wallets,
                utxos,
                batches,
                payouts,
            )
            .await;
            spawn_batch_broadcasting(pool.begin().await?, data.clone()).await?;
            res
        })
        .await?;
    Ok(())
}

#[job(name = "batch_signing", channel_name = "batch_signing")]
#[allow(clippy::too_many_arguments)]
async fn batch_signing(
    mut current_job: CurrentJob,
    JobsConfig { signing, .. }: JobsConfig,
    blockchain_cfg: BlockchainConfig,
    signer_encryption_config: SignerEncryptionConfig,
    batches: Batches,
    wallets: Wallets,
    xpubs: XPubs,
    signing_sessions: SigningSessions,
) -> Result<(), JobError> {
    let pool = current_job.pool().clone();
    JobExecutor::builder(&mut current_job)
        .warn_retries(signing.warn_retries)
        .max_attempts(signing.max_attempts)
        .max_retry_delay(signing.max_retry_delay)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: BatchSigningData = data.expect("no BatchSigningData available");
            let (data, complete) = batch_signing::execute(
                pool.clone(),
                data,
                blockchain_cfg,
                batches,
                signing_sessions,
                wallets,
                xpubs,
                signer_encryption_config,
            )
            .await?;

            if complete {
                spawn_batch_broadcasting(pool.begin().await?, data.clone()).await?;
            }

            Ok::<_, JobError>(data)
        })
        .await?;
    Ok(())
}

#[job(
    name = "batch_broadcasting",
    channel_name = "batch_broadcasting",
    retries = 20,
    ordered = true
)]
async fn batch_broadcasting(
    mut current_job: CurrentJob,
    blockchain_cfg: BlockchainConfig,
    batches: Batches,
) -> Result<(), JobError> {
    JobExecutor::builder(&mut current_job)
        .build()
        .expect("couldn't build JobExecutor")
        .execute(|data| async move {
            let data: BatchBroadcastingData = data.expect("no BatchBroadcastingData available");
            batch_broadcasting::execute(data, blockchain_cfg, batches).await
        })
        .await?;
    Ok(())
}

#[instrument(name = "job.spawn_sync_all_wallets", skip_all, fields(error, error.level, error.message), err)]
pub async fn spawn_sync_all_wallets(
    pool: &sqlx::PgPool,
    duration: std::time::Duration,
) -> Result<(), JobError> {
    match JobBuilder::new_with_id(SYNC_ALL_WALLETS_ID, "sync_all_wallets")
        .set_channel_name("sync_all_wallets")
        .set_delay(duration)
        .spawn(pool)
        .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(()),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(name = "job.spawn_sync_wallet", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_sync_wallet(pool: &sqlx::PgPool, data: SyncWalletData) -> Result<(), JobError> {
    onto_account_main_channel(pool, data.account_id, data.wallet_id, "sync_wallet", data).await?;
    Ok(())
}

#[instrument(name = "job.spawn_process_all_payout_queues", skip_all, fields(error, error.level, error.message), err)]
pub async fn spawn_process_all_payout_queues(
    pool: &sqlx::PgPool,
    delay: std::time::Duration,
) -> Result<(), JobError> {
    match JobBuilder::new_with_id(PROCESS_ALL_PAYOUT_QUEUES_ID, "process_all_payout_queues")
        .set_channel_name("process_all_payout_queues")
        .set_delay(delay)
        .spawn(pool)
        .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(()),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(name = "job.schedule_spawn_process_payout_queue", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_schedule_process_payout_queue(
    pool: &sqlx::PgPool,
    data: impl Into<ProcessPayoutQueueData>,
    delay: std::time::Duration,
) -> Result<(), JobError> {
    let data = data.into();
    match JobBuilder::new_with_id(
        Uuid::from(data.payout_queue_id),
        "schedule_process_payout_queue",
    )
    .set_ordered(true)
    .set_channel_name("schedule_payout_queue")
    .set_channel_args(&schedule_payout_queue_channel_arg(data.payout_queue_id))
    .set_delay(delay)
    .set_json(&data)
    .expect("Couldn't set json")
    .spawn(pool)
    .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(()),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(name = "job.spawn_batch_wallet_accounting", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_batch_wallet_accounting(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    data: impl Into<BatchWalletAccountingData>,
) -> Result<(), JobError> {
    let data = data.into();
    match batch_wallet_accounting
        .builder()
        .set_json(&data)
        .expect("Couldn't set json")
        .set_channel_args(&format!("wallet_id:{}", data.wallet_id))
        .spawn(&mut **tx)
        .await
    {
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

#[instrument(name = "job.spawn_batch_signing", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_batch_signing(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    data: impl Into<BatchSigningData>,
) -> Result<(), JobError> {
    let data = data.into();
    match batch_signing
        .builder()
        .set_json(&data)
        .expect("Couldn't set json")
        .set_ordered(true)
        .set_channel_args(&format!("batch_id:{}", data.batch_id))
        .spawn(&mut *tx)
        .await
    {
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => {
            tx.commit().await?;
            Ok(())
        }
    }
}

#[instrument(name = "job.spawn_all_batch_signings", skip_all, fields(error, error.level, error.message), err)]
pub async fn spawn_all_batch_signings(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    jobs: impl Iterator<Item = impl Into<BatchSigningData>>,
) -> Result<(), JobError> {
    for job in jobs {
        let data = job.into();
        batch_signing
            .builder()
            .set_json(&data)
            .expect("Couldn't set json")
            .set_ordered(true)
            .set_channel_args(&format!("batch_id:{}", data.batch_id))
            .spawn(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[instrument(name = "job.spawn_batch_broadcasting", skip_all, fields(error, error.level, error.message), err)]
async fn spawn_batch_broadcasting(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    data: impl Into<BatchBroadcastingData>,
) -> Result<(), JobError> {
    let data = data.into();
    match batch_broadcasting
        .builder()
        .set_json(&data)
        .expect("Couldn't set json")
        .set_channel_args(&format!("batch_id:{}", data.batch_id))
        .spawn(&mut *tx)
        .await
    {
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::WARN, &e);
            Err(e.into())
        }
        Ok(_) => {
            tx.commit().await?;
            Ok(())
        }
    }
}

#[instrument(name = "job.spawn_outbox_handler", skip_all)]
pub async fn spawn_outbox_handler(pool: &sqlx::PgPool, account: Account) -> Result<(), JobError> {
    let data = PopulateOutboxData {
        account_id: account.id,
        journal_id: account.journal_id(),
        tracing_data: crate::tracing::extract_tracing_data(),
    };
    match JobBuilder::new_with_id(Uuid::from(data.journal_id), "populate_outbox")
        .set_channel_name("populate_outbox")
        .set_channel_args(&format!("account_id:{}", data.account_id))
        .set_json(&data)
        .expect("Couldn't set json")
        .spawn(pool)
        .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(()),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}
#[instrument(name = "job.spawn_respawn_all_outbox_handlers", skip_all, fields(error, error.level, error.message), err)]
pub async fn spawn_respawn_all_outbox_handlers(
    pool: &sqlx::PgPool,
    duration: std::time::Duration,
) -> Result<(), JobError> {
    match JobBuilder::new_with_id(RESPAWN_ALL_OUTBOX_ID, "respawn_all_outbox_handlers")
        .set_channel_name("respawn_all_outbox_handlers")
        .set_delay(duration)
        .spawn(pool)
        .await
    {
        Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => Ok(()),
        Err(e) => {
            crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
            Err(e.into())
        }
        Ok(_) => Ok(()),
    }
}

fn schedule_payout_queue_channel_arg(payout_queue_id: PayoutQueueId) -> String {
    format!("payout_queue_id:{payout_queue_id}")
}

async fn onto_account_main_channel<D: serde::Serialize>(
    pool: &sqlx::PgPool,
    account_id: AccountId,
    uuid: impl Into<Uuid>,
    name: &str,
    data: D,
) -> Result<D, JobError> {
    let uuid = uuid.into();
    loop {
        match JobBuilder::new_with_id(uuid, name)
            .set_ordered(true)
            .set_retry_backoff(std::time::Duration::from_secs(2))
            .set_channel_name("account_main")
            .set_channel_args(&account_main_channel_arg(account_id))
            .set_json(&data)
            .expect("Couldn't set json")
            .spawn(pool)
            .await
        {
            Err(sqlx::Error::Database(err)) if err.message().contains("duplicate key") => {
                return Ok(data)
            }
            Err(sqlx::Error::Database(err)) if err.message().contains("after_message_id_fkey") => {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
            Err(e) => {
                crate::tracing::insert_error_fields(tracing::Level::ERROR, &e);
                return Err(JobError::from(e));
            }
            Ok(_) => return Ok(data),
        }
    }
}

fn account_main_channel_arg(account_id: AccountId) -> String {
    format!("account_id:{account_id}")
}

impl From<(AccountId, PayoutQueueId)> for ProcessPayoutQueueData {
    fn from((account_id, payout_queue_id): (AccountId, PayoutQueueId)) -> Self {
        Self {
            payout_queue_id,
            account_id,
            batch_id: BatchId::new(),
            tracing_data: crate::tracing::extract_tracing_data(),
        }
    }
}

impl From<(&ProcessPayoutQueueData, WalletId)> for BatchWalletAccountingData {
    fn from((data, wallet_id): (&ProcessPayoutQueueData, WalletId)) -> Self {
        Self {
            tracing_data: crate::tracing::extract_tracing_data(),
            account_id: data.account_id,
            batch_id: data.batch_id,
            wallet_id,
        }
    }
}

impl From<&ProcessPayoutQueueData> for BatchSigningData {
    fn from(data: &ProcessPayoutQueueData) -> Self {
        Self {
            account_id: data.account_id,
            batch_id: data.batch_id,
            tracing_data: crate::tracing::extract_tracing_data(),
        }
    }
}
impl From<(AccountId, BatchId)> for BatchSigningData {
    fn from((account_id, batch_id): (AccountId, BatchId)) -> Self {
        Self {
            batch_id,
            account_id,
            tracing_data: crate::tracing::extract_tracing_data(),
        }
    }
}

impl From<BatchWalletAccountingData> for BatchBroadcastingData {
    fn from(data: BatchWalletAccountingData) -> Self {
        Self {
            account_id: data.account_id,
            batch_id: data.batch_id,
            tracing_data: crate::tracing::extract_tracing_data(),
        }
    }
}

impl From<BatchSigningData> for BatchBroadcastingData {
    fn from(data: BatchSigningData) -> Self {
        Self {
            account_id: data.account_id,
            batch_id: data.batch_id,
            tracing_data: crate::tracing::extract_tracing_data(),
        }
    }
}
