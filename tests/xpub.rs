mod helpers;

use bria::{app::*, xpub::*};

#[tokio::test]
async fn test_xpub() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;

    let profile = helpers::create_test_account(&pool).await?;

    let original = "tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4";
    let xpub = XPub::try_from((original, Some("m/84'/0'/0'"))).unwrap();
    let repo = XPubs::new(&pool);
    let _ = repo
        .persist(
            NewAccountXPub::builder()
                .account_id(profile.account_id)
                .original(original.to_owned())
                .key_name("name")
                .value(xpub)
                .build()
                .unwrap(),
        )
        .await?;
    Ok(())
}

#[tokio::test]
async fn rotate_encryption_key() -> anyhow::Result<()> {
    let pool = helpers::init_pool().await?;
    let profile = helpers::create_test_account(&pool).await?;
    let app = App::run(pool.clone(), AppConfig::default()).await?;
    let repo = XPubs::new(&pool);
    app.import_xpub(profile.clone(), "test".to_string(), "tpubDD4vFnWuTMEcZiaaZPgvzeGyMzWe6qHW8gALk5Md9kutDvtdDjYFwzauEFFRHgov8pAwup5jX88j5YFyiACsPf3pqn5hBjvuTLRAseaJ6b4".to_string(), Some("m/84h/0h/0h".to_string())).await?;
    let xpub = repo
        .find_from_ref(profile.account_id, XPubRef::Name("test".to_string()))
        .await?;
    app.set_signer_config(
        profile,
        "test".to_string(),
        SignerConfig::Bitcoind(BitcoindSignerConfig {
            endpoint: "https://localhost:18543".to_string(),
            rpc_user: "rpcuser".to_string(),
            rpc_password: "password".to_string(),
        }),
    )
    .await?;
    let new_encryption_key =
        "04b37b3f6c7e751eb9940dcf619613b8d72dae2fa43fa795c27217bbad61d47f".to_string();
    let key_bytes = hex::decode(new_encryption_key)?;
    let mut app_cfg = AppConfig::default();
    let encryption_key = EncryptionKey::clone_from_slice(key_bytes.as_ref());
    app_cfg.signer_encryption.key = encryption_key;
    let nonce = "30aa686230c68391c5c3952d".to_string();
    let old_key = "715cd5e6fdd8179779f996fe7f09a379af3e2182c43646cc07609e1365a8b443b1234659e6c4ec09e944fd6db8ea1b94".to_string();
    let deprecated_key = DeprecatedEncryptionKey {
        nonce,
        key: old_key,
    };
    let app = App::run(pool, app_cfg).await?;
    app.rotate_encryption_key(&deprecated_key).await?;
    let _ = xpub.signing_cfg(encryption_key);

    Ok(())
}
