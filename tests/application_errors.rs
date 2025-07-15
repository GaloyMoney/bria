mod helpers;

use es_entity::EsEntityError;
use rand::distributions::{Alphanumeric, DistString};

use bria::{
    address::error::AddressError,
    app::{error::ApplicationError, *},
    payout_queue::error::PayoutQueueError,
    primitives::*,
    profile::error::ProfileError,
    wallet::error::WalletError,
};

#[tokio::test]
async fn external_id_does_not_exist() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;

    let app = App::run(pool, AppConfig::default()).await?;
    let err = app
        .find_address_by_external_id(&profile, "external_id".to_string())
        .await;

    assert!(matches!(
        err,
        Err(ApplicationError::AddressError(AddressError::EsEntityError(
            EsEntityError::NotFound
        )))
    ));

    Ok(())
}

#[tokio::test]
async fn external_id_already_exists() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;

    let external = "wpkh([1ff51810/84'/0'/0']tpubDDdzmt7vndmNywiVAeBPuhYLTFa7hmtfaqUxxTv5iLy7bxU93B62M9WKFSmn1BEN2vte8GDD3SUNKbupRajFW4RK8hd3i6W15pvTRQfo1fK/0/*)#q8r69l4d".to_owned();
    let internal = "wpkh([1ff51810/84'/0'/0']tpubDDdzmt7vndmNywiVAeBPuhYLTFa7hmtfaqUxxTv5iLy7bxU93B62M9WKFSmn1BEN2vte8GDD3SUNKbupRajFW4RK8hd3i6W15pvTRQfo1fK/1/*)#3nxmc294".to_owned();
    let app = App::run(pool, AppConfig::default()).await?;
    let wallet_name = "test_import_descriptor".to_owned();
    let _ = app
        .create_descriptors_wallet(&profile, wallet_name.clone(), external, internal)
        .await?;

    let external_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let addr = app
        .new_address(
            &profile,
            wallet_name.clone(),
            Some(external_id.clone()),
            None,
        )
        .await;
    assert!(addr.is_ok());
    let addr = app
        .new_address(&profile, wallet_name, Some(external_id), None)
        .await;
    assert!(matches!(
        addr,
        Err(ApplicationError::AddressError(
            AddressError::ExternalIdAlreadyExists
        ))
    ));
    Ok(())
}

#[tokio::test]
async fn profile_key_not_found() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let key = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let app = App::run(pool, AppConfig::default()).await?;
    let profile_err = app.authenticate(&key).await;
    assert!(matches!(
        profile_err,
        Err(ApplicationError::ProfileError(
            ProfileError::ProfileKeyNotFound
        ))
    ));
    Ok(())
}

#[tokio::test]
async fn payout_queue_id_not_found() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;
    let app = App::run(pool, AppConfig::default()).await?;
    let payout_queue_id = PayoutQueueId::new();
    let err = app
        .update_payout_queue(&profile, payout_queue_id, None, None)
        .await;
    assert!(matches!(
        err,
        Err(ApplicationError::PayoutQueueError(
            PayoutQueueError::EsEntityError(EsEntityError::NotFound)
        ))
    ));
    Ok(())
}

#[tokio::test]
async fn wallet_name_not_found() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;
    let app = App::run(pool, AppConfig::default()).await?;
    let wallet_name = "test".to_string();
    let err = app.new_address(&profile, wallet_name, None, None).await;
    assert!(matches!(
        err,
        Err(ApplicationError::WalletError(WalletError::EsEntityError(
            EsEntityError::NotFound
        )))
    ));
    Ok(())
}

#[tokio::test]
async fn payout_queue_name_not_found() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;
    let external = "wpkh([1ff51810/84'/0'/0']tpubDDdzmt7vndmNywiVAeBPuhYLTFa7hmtfaqUxxTv5iLy7bxU93B62M9WKFSmn1BEN2vte8GDD3SUNKbupRajFW4RK8hd3i6W15pvTRQfo1fK/0/*)#q8r69l4d".to_owned();
    let internal = "wpkh([1ff51810/84'/0'/0']tpubDDdzmt7vndmNywiVAeBPuhYLTFa7hmtfaqUxxTv5iLy7bxU93B62M9WKFSmn1BEN2vte8GDD3SUNKbupRajFW4RK8hd3i6W15pvTRQfo1fK/1/*)#3nxmc294".to_owned();
    let app = App::run(pool, AppConfig::default()).await?;
    let wallet_name = "test_wallet".to_owned();
    let _ = app
        .create_descriptors_wallet(&profile, wallet_name.clone(), external, internal)
        .await?;
    let address = Address::parse_from_trusted_source("3EZQk4F8GURH5sqVMLTFisD17yNeKa7Dfs");
    let queue_name = "test".to_string();
    let sats = Satoshis::from(10000);
    let err = app
        .estimate_payout_fee_to_address(
            &profile,
            wallet_name,
            queue_name,
            address.to_string(),
            sats,
        )
        .await;
    assert!(matches!(
        err,
        Err(ApplicationError::PayoutQueueError(
            PayoutQueueError::EsEntityError(EsEntityError::NotFound)
        ))
    ));

    Ok(())
}

#[tokio::test]
async fn profile_name_not_found() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;
    let app = App::run(pool, AppConfig::default()).await?;
    let err = app
        .create_profile_api_key(&profile, "test".to_string())
        .await;
    assert!(matches!(
        err,
        Err(ApplicationError::ProfileError(ProfileError::EsEntityError(
            EsEntityError::NotFound
        )))
    ));
    Ok(())
}
