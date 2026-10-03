use clap::Args;
use clap::builder::NonEmptyStringValueParser;

#[derive(Debug, Args)]
pub struct Schema {
    /// Print the introspection result as JSON instead of SDL
    #[arg(long)]
    pub json: bool,
    /// Write the schema to this file instead of stdout
    #[arg(long, short, value_name = "FILE", value_parser = NonEmptyStringValueParser::new())]
    pub output: Option<String>,
}
