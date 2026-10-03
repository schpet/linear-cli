use std::path::PathBuf;

use clap::{Args, ValueHint};

#[derive(Debug, Args)]
pub struct Schema {
    /// Print the introspection result as JSON instead of SDL
    #[arg(long, short)]
    pub json: bool,
    /// Write the schema to this file instead of stdout
    #[arg(long, short, value_name = "FILE", value_hint = ValueHint::FilePath)]
    pub output: Option<PathBuf>,
}
