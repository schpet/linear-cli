use clap::{Args, Subcommand};

use super::LINEAR_MARKDOWN;

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct InitiativeUpdate {
    #[command(subcommand)]
    pub command: InitiativeUpdateCommand,
}

#[derive(Debug, Subcommand)]
pub enum InitiativeUpdateCommand {
    /// Post a status update on an initiative
    #[command(alias = "c", after_long_help = LINEAR_MARKDOWN)]
    Create(InitiativeUpdateCreate),
    /// List an initiative's status updates
    #[command(aliases = ["l", "ls"])]
    List(InitiativeUpdateList),
}

#[derive(Debug, Args)]
pub struct InitiativeUpdateCreate {
    /// Initiative ID, slug, or name
    #[arg(value_name = "INITIATIVE")]
    pub initiative_id: String,
    #[command(flatten)]
    pub update: super::project_update::StatusUpdateArgs,
}

#[derive(Debug, Args)]
pub struct InitiativeUpdateList {
    /// Initiative ID, slug, or name
    #[arg(value_name = "INITIATIVE")]
    pub initiative_id: String,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
    /// Maximum number of updates to show, newest first (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "10")]
    pub limit: super::Limit,
}
