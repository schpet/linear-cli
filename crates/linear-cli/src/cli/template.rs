use clap::builder::NonEmptyStringValueParser;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
#[command(arg_required_else_help = true)]
pub struct Template {
    #[command(subcommand)]
    pub command: TemplateCommand,
}

#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    /// List templates
    ///
    /// Without --team, every template in the workspace is listed.
    List(TemplateList),
    /// Show a template and the fields it fills in
    #[command(visible_alias = "v")]
    View(TemplateView),
}

#[derive(Debug, Args)]
pub struct TemplateList {
    /// Show only templates of this type
    #[arg(long = "type")]
    pub r#type: Option<super::TemplateType>,
    /// Show this team's templates (key, name, or ID) plus workspace templates
    #[arg(long, value_parser = NonEmptyStringValueParser::new())]
    pub team: Option<String>,
    /// Maximum number of templates to show (a number or `all`)
    #[arg(long, value_parser = super::limit::parse, default_value = "all")]
    pub limit: super::Limit,
    /// Print JSON
    #[arg(long, short)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct TemplateView {
    /// Template name or ID
    pub template: String,
    /// Print JSON
    ///
    /// `templateData` stays a JSON-encoded string; decode it with
    /// `jq '.templateData | fromjson'`.
    #[arg(long, short)]
    pub json: bool,
}
