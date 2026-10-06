use clap::Args;

use super::values::NonBlank;

#[derive(Debug, Args)]
pub struct Api {
    /// GraphQL query or mutation; read from stdin when omitted
    #[arg(value_name = "DOCUMENT")]
    pub graphql_document: Option<String>,
    /// Set a variable; repeatable
    ///
    /// A value that looks like a boolean, number, or null is sent as one, and
    /// `@path` reads the value from a file.
    #[arg(long, value_name = "KEY=VALUE", value_parser = super::variable_assignment)]
    pub variable: Vec<super::VariableAssignment>,
    /// Variables as a JSON object; --variable takes precedence
    #[arg(long, value_name = "JSON", value_parser = NonBlank)]
    pub variables_json: Option<String>,
    /// Follow the cursor of the one connection in the response and print every page
    #[arg(long)]
    pub paginate: bool,
    /// Print nothing; the exit status still says whether and why it failed
    /// (see `linear --help`)
    #[arg(long)]
    pub silent: bool,
}
