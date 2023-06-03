mod helpers;

use rand::distributions::{Alphanumeric, DistString};

use bria::{
    address::error::AddressError,
    app::{error::ApplicationError, *},
};

#[tokio::test]
async fn external_id_does_not_exist() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;

    let app = App::run(pool, AppConfig::default()).await?;
    let err = app
        .find_address_by_external_id(profile, "external_id".to_string())
        .await;

    assert!(matches!(
        err,
        Err(ApplicationError::AddressError(
            AddressError::ExternalIdNotFound
        ))
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
        .create_descriptors_wallet(profile.clone(), wallet_name.clone(), external, internal)
        .await?;

    let external_id = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let addr = app
        .new_address(
            profile.clone(),
            wallet_name.clone(),
            Some(external_id.clone()),
            None,
        )
        .await;
    assert!(matches!(addr, Ok(_)));
    let addr = app
        .new_address(profile, wallet_name, Some(external_id), None)
        .await;
    assert!(matches!(
        addr,
        Err(ApplicationError::AddressError(
            AddressError::ExternalIdAlreadyExists
        ))
    ));
    Ok(())
}
