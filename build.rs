fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=migrations");

    if std::env::var("PROTOC").ok().is_some() {
        println!("Using PROTOC set in environment.");
    } else {
        println!("Setting PROTOC to protoc-bin-vendored version.");
        std::env::set_var("PROTOC", protobuf_src::protoc());
    }

    tonic_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize)]")
        .type_attribute(".", "#[serde(rename_all = \"camelCase\")]")
        .extern_path(".google.protobuf.Struct", "::prost_wkt_types::Struct")
        .compile(&["proto/api/bria.proto"], &["proto"])?;

    tonic_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize)]")
        .type_attribute(".", "#[serde(rename_all = \"camelCase\")]")
        .compile(&["proto/admin/api.proto"], &["proto"])?;

    Ok(())
}
