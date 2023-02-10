fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=migrations");

    std::env::set_var("PROTOC", protobuf_src::protoc());

    tonic_build::configure()
        .type_attribute(
            "GetWalletBalanceSummaryResponse",
            "#[derive(serde::Deserialize, serde::Serialize)]",
        )
        .compile(&["proto/api/bria.proto"], &["proto"])?;

    tonic_build::compile_protos("proto/admin/api.proto")?;

    Ok(())
}
