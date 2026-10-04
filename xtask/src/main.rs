//! Repository maintenance tasks, run with `cargo xtask <task>` (an alias in
//! `.cargo/config.toml`).

#![forbid(unsafe_code)]
#![deny(
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod licenses;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "xtask",
    bin_name = "cargo xtask",
    about = "Repository maintenance tasks"
)]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Write the license notices of every third-party crate in Cargo.lock to
    /// one Markdown file, as shipped in release archives
    Licenses {
        /// Where to write the notices [default: THIRD_PARTY_LICENSES.md in the
        /// workspace root]
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives in a directory of the workspace root");
    let result: Result<(), Box<dyn Error>> = match cli.task {
        Task::Licenses { output } => licenses::run(workspace, output),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
