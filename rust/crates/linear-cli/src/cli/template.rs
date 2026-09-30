use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct Template {
    #[command(subcommand)]
    pub command: Option<TemplateCommand>,
}

#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    #[command(
        name = "list",
        about = "List templates. Without --team, every template in the workspace is shown."
    )]
    List(TemplateList),
    #[command(name = "view", about = "Show a template and what it pre-fills. Pass its name or ID.", visible_aliases = ["v"])]
    View(TemplateView),
}

#[derive(Debug, Args)]
pub struct TemplateList {
    #[arg(
        long = "type",
        help = "Only templates of this type (issue, project, or document)",
        value_name = "type"
    )]
    pub r#type: Option<super::TemplateType>,
    #[arg(long = "team", help = "Team key, name, or ID. Shows that team's templates plus workspace templates.", value_name = "team", value_parser = super::nonempty_string)]
    pub team: Option<String>,
    #[arg(long = "json", short = 'j', help = "Output as JSON")]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct TemplateView {
    #[arg(value_name = "template")]
    pub template: String,
    #[arg(
        long = "json",
        short = 'j',
        help = "Output the template as JSON (templateData stays a JSON-encoded string; use `jq '.templateData | fromjson'`)"
    )]
    pub json: bool,
}
