use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Cycle {
    #[command(subcommand)]
    pub command: CycleCommand,
}

#[derive(Debug, Subcommand)]
pub enum CycleCommand {
    /// List a team's cycles
    List(CycleList),
    /// Show a cycle and its issues
    #[command(visible_alias = "v")]
    View(CycleView),
}

#[derive(Debug, Args)]
pub struct CycleList {
    /// Team key, name, or ID; defaults to the configured team
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Maximum number of cycles to show, newest first (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct CycleView {
    /// Cycle name, number, `active`, `next`, `previous`, or an offset like +1
    #[arg(value_name = "CYCLE")]
    pub cycle_ref: String,
    /// Team key, name, or ID; defaults to the configured team
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}
