use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct Cycle {
    #[command(subcommand)]
    pub command: Option<CycleCommand>,
}

#[derive(Debug, Subcommand)]
pub enum CycleCommand {
    #[command(name = "list", about = "List cycles for a team")]
    List(CycleList),
    #[command(name = "view", about = "View cycle details", visible_aliases = ["v"])]
    View(CycleView),
}

#[derive(Debug, Args)]
pub struct CycleList {
    #[arg(long = "team", help = "Team key, name, or ID (defaults to current team)", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct CycleView {
    #[arg(value_name = "cycleRef")]
    pub cycle_ref: String,
    #[arg(long = "team", help = "Team key, name, or ID (defaults to current team)", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}
