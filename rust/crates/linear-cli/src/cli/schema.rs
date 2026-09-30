use clap::Args;

#[derive(Debug, Args)]
pub struct Schema {
    #[arg(
        long = "json",
        help = "Output as JSON introspection result instead of SDL"
    )]
    pub json: bool,
    #[arg(long = "output", short = 'o', help = "Write schema to file instead of stdout", value_name = "file", value_parser = super::nonempty_string)]
    pub output: Option<String>,
}
