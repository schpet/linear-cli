#![forbid(unsafe_code)]

use std::process::ExitCode;

use clap::Parser;
use linear_cli::cli::Cli;

fn main() -> ExitCode {
    // Help, version and usage errors never read configuration.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    };
    ExitCode::from(linear_cli::app::main(cli))
}
