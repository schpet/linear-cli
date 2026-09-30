use clap::Args;

#[derive(Debug, Args)]
pub struct Api {
    #[arg(value_name = "graphqlDocument")]
    pub graphql_document: Option<String>,
    #[arg(long = "variable", help = "Variable in key=value format (coerces booleans, numbers, null; @file reads from path)", value_name = "variable", value_parser = super::variable_assignment)]
    pub variable: Vec<super::VariableAssignment>,
    #[arg(long = "variables-json", help = "JSON object of variables (merged with --variable, which takes precedence)", value_name = "json", value_parser = super::nonempty_string)]
    pub variables_json: Option<String>,
    #[arg(
        long = "paginate",
        help = "Auto-paginate a single connection field using cursor pagination"
    )]
    pub paginate: bool,
    #[arg(
        long = "silent",
        help = "Suppress response output (exit code still reflects errors)"
    )]
    pub silent: bool,
}
