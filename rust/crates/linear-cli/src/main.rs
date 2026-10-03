#![forbid(unsafe_code)]

use std::process::ExitCode;

use clap::Parser;
use linear_cli::cli::Cli;

fn main() -> ExitCode {
    // A shell asking for completions (COMPLETE=<shell>) is answered from the
    // command grammar alone, before anything else runs.
    clap_complete::CompleteEnv::with_factory(linear_cli::cli::command).complete();
    // Help, version and usage errors never read configuration.
    ExitCode::from(linear_cli::app::main(Cli::parse()))
}
