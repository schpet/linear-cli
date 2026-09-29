//! Registers the committed Linear SDL with Cynic at build time.
//!
//! The schema at `graphql/schema.graphql` is the only maintained SDL. It is read
//! through a `CARGO_MANIFEST_DIR`-rooted path and never copied or fetched.

use std::path::PathBuf;

fn main() {
    let manifest_dir = match std::env::var_os("CARGO_MANIFEST_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => {
            println!("cargo::error=CARGO_MANIFEST_DIR is not set");
            return;
        }
    };
    let schema_path = manifest_dir.join("../../../graphql/schema.graphql");
    println!("cargo::rerun-if-changed={}", schema_path.display());
    println!("cargo::rerun-if-changed=build.rs");
    if let Err(error) = cynic_codegen::register_schema("linear").from_sdl_file(&schema_path) {
        println!(
            "cargo::error=failed to register Linear schema from {}: {error}",
            schema_path.display()
        );
    }
}
