//! Registers the Linear GraphQL schema in `graphql/schema.graphql` with cynic.

use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"),
    );
    let schema_path = manifest_dir.join("../../graphql/schema.graphql");
    println!("cargo::rerun-if-changed={}", schema_path.display());
    println!("cargo::rerun-if-changed=build.rs");
    if let Err(error) = cynic_codegen::register_schema("linear").from_sdl_file(&schema_path) {
        println!(
            "cargo::error=failed to register Linear schema from {}: {error}",
            schema_path.display()
        );
    }
}
