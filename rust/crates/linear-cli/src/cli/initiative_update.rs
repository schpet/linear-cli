use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct InitiativeUpdate {
    #[command(subcommand)]
    pub command: InitiativeUpdateCommand,
}

#[derive(Debug, Subcommand)]
pub enum InitiativeUpdateCommand {
    #[command(name = "create", about = "Create a new status update for an initiative", long_about = "Create a new status update for an initiative\n\nLinear Markdown: a plain Linear URL creates a mention; `@name`, `@[Name](id)`,\nand `[Name](url)` do not. Get a person's URL from the `url` field of\n`linear team members <TEAM> --json`, or an issue's from `linear issue url <ID>`.\nRun `linear markdown` for collapsible sections and the full reference.", visible_aliases = ["c"])]
    Create(InitiativeUpdateCreate),
    #[command(name = "list", about = "List status updates for an initiative", visible_aliases = ["l", "ls"])]
    List(InitiativeUpdateList),
}

#[derive(Debug, Args)]
pub struct InitiativeUpdateCreate {
    #[arg(value_name = "initiativeId")]
    pub initiative_id: String,
    #[command(flatten)]
    pub update: super::project_update::StatusUpdateArgs,
}

#[derive(Debug, Args)]
pub struct InitiativeUpdateList {
    #[arg(value_name = "initiativeId")]
    pub initiative_id: String,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
    #[arg(long = "limit", help = "Limit results", value_name = "limit", value_parser = clap::value_parser!(i32).range(1..), default_value = "10")]
    pub limit: i32,
}
