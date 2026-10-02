//! `linear schema`: print the API's GraphQL schema as SDL or introspection JSON.
use crate::{
    cli::schema::Schema,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::{
        bulk_error,
        envelope::GraphQlRequest,
        schema_introspection::{Model, QUERY},
        transport::{GraphQlTransport, classify_typed},
    },
};
pub fn run(ctx: &Ctx, args: &Schema) -> Result<()> {
    write_schema(ctx, args).context("Failed to fetch schema")
}

fn write_schema(ctx: &Ctx, args: &Schema) -> Result<()> {
    let client = ctx.client()?;
    let value = ctx.spin(true, fetch(client))?;
    let content = format!("{}\n", content(&value, args.json)?);
    match &args.output {
        Some(path) => {
            std::fs::write(path, content).map_err(|error| {
                Error::new(format!("Failed to write schema: {path}: {error}")).with_source(error)
            })?;
            ctx.print(format!("Schema written to {path}\n"))
        }
        None => ctx.print(content),
    }
}

async fn fetch(transport: &GraphQlTransport) -> Result<serde_json::Value> {
    let request: GraphQlRequest<()> = GraphQlRequest {
        query: QUERY.to_owned(),
        variables: None,
        operation_name: Some("IntrospectionQuery".into()),
    };
    let response = transport.send_request(&request).await?;
    if let Some(error) =
        bulk_error::observe_source_error(&response, &request).map_err(|e| e.into_error())?
    {
        return Err(Error::new(error.preferred_message.unwrap_or(error.message)));
    }
    classify_typed(response).map_err(Error::from)
}
fn content(value: &serde_json::Value, json: bool) -> Result<String> {
    if json {
        Ok(serde_json::to_string_pretty(value).expect("a JSON value always serializes"))
    } else {
        Model::parse(value)?.print()
    }
}
