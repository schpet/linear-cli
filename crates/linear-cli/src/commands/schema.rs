//! `linear schema`: print the API's GraphQL schema as SDL or introspection JSON.
//!
//! Linear rejects queries above a complexity budget, and a standard
//! introspection query (every type with its fields and deeply nested type
//! references) is far over it. The schema is fetched in two steps instead:
//! the type names and directives, then the full definition of each type in
//! batches of aliased `__type(name:)` lookups, which Linear prices per type.
//! The pieces are assembled into the shape a standard introspection query
//! returns.
use std::collections::HashMap;

use cynic_introspection::{IntrospectionQuery, Type};
use futures_util::{StreamExt, TryStreamExt, stream};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{
    cli::schema::Schema,
    client::LinearClient,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::envelope::GraphQlRequest,
};

/// Types per request. One type costs about 92 complexity points against
/// Linear's limit of 10,000 per request.
const TYPES_PER_REQUEST: usize = 50;

/// Requests in flight at once.
const CONCURRENT_REQUESTS: usize = 4;

/// The root types, every type name, and the directives. Lists nested under
/// `__schema` multiply the price of everything below them, so directive
/// argument types are selected two levels deep, which covers `String!`.
const OVERVIEW: &str = "query SchemaOverview { __schema { \
    queryType { name } mutationType { name } subscriptionType { name } \
    types { name } \
    directives { name description locations args { name description type { kind name ofType { kind name } } defaultValue } } \
} }";

const FRAGMENTS: &str = "\
fragment TypeRef on __Type { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name ofType { kind name } } } } } } } } \
fragment InputValue on __InputValue { name description type { ...TypeRef } defaultValue } \
fragment FullType on __Type { kind name description \
fields(includeDeprecated: true) { name description args { ...InputValue } type { ...TypeRef } isDeprecated deprecationReason } \
inputFields { ...InputValue } interfaces { name } \
enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason } \
possibleTypes { name } }";

pub fn run(ctx: &Ctx, args: &Schema) -> Result<()> {
    write_schema(ctx, args).context("Failed to fetch schema")
}

fn write_schema(ctx: &Ctx, args: &Schema) -> Result<()> {
    let client = ctx.client()?;
    let data = ctx.spin(true, introspect(client))?;
    // Parsing checks the result is a complete schema even when only the
    // JSON is printed.
    let schema = parse(&data)?;
    let content = if args.json {
        serde_json::to_string_pretty(&data).expect("a JSON value always serializes")
    } else {
        sdl(schema)
    };
    let content = format!("{}\n", content.trim_end());
    match &args.output {
        Some(path) => {
            std::fs::write(path, content).map_err(|error| {
                Error::new(format!(
                    "Failed to write schema: {}: {error}",
                    path.display()
                ))
                .with_source(error)
            })?;
            ctx.print(format!("Schema written to {}\n", path.display()))
        }
        None => ctx.print(content),
    }
}

#[derive(Deserialize)]
struct Overview {
    #[serde(rename = "__schema")]
    schema: OverviewSchema,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverviewSchema {
    query_type: Value,
    mutation_type: Value,
    subscription_type: Value,
    types: Vec<TypeName>,
    directives: Vec<Value>,
}

#[derive(Deserialize)]
struct TypeName {
    name: String,
}

/// The data of a standard introspection query: `{"__schema": {...}}`.
async fn introspect(client: &LinearClient) -> Result<Value> {
    let overview: Overview = client
        .execute_request(&request(OVERVIEW.to_owned(), None))
        .await?;
    let names: Vec<String> = overview
        .schema
        .types
        .into_iter()
        .map(|ty| ty.name)
        .collect();
    let batches: Vec<Vec<Value>> = stream::iter(names.chunks(TYPES_PER_REQUEST))
        .map(|batch| fetch_types(client, batch))
        .buffered(CONCURRENT_REQUESTS)
        .try_collect()
        .await?;
    Ok(json!({
        "__schema": {
            "queryType": overview.schema.query_type,
            "mutationType": overview.schema.mutation_type,
            "subscriptionType": overview.schema.subscription_type,
            "types": batches.into_iter().flatten().collect::<Vec<_>>(),
            "directives": overview.schema.directives,
        }
    }))
}

fn request(query: String, variables: Option<Value>) -> GraphQlRequest {
    GraphQlRequest {
        query,
        variables,
        operation_name: None,
    }
}

/// The document looking up `count` types, named by the variables `$t0`,
/// `$t1`, ... under the aliases `t0`, `t1`, ...
fn types_document(count: usize) -> String {
    let variables: Vec<String> = (0..count).map(|i| format!("$t{i}: String!")).collect();
    let lookups: Vec<String> = (0..count)
        .map(|i| format!("t{i}: __type(name: $t{i}) {{ ...FullType }}"))
        .collect();
    format!(
        "query SchemaTypes({}) {{ {} }} {FRAGMENTS}",
        variables.join(", "),
        lookups.join(" ")
    )
}

/// The full definitions of `names`, in the same order.
async fn fetch_types(client: &LinearClient, names: &[String]) -> Result<Vec<Value>> {
    let variables: Map<String, Value> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (format!("t{i}"), Value::String(name.clone())))
        .collect();
    let mut data: HashMap<String, Value> = client
        .execute_request(&request(
            types_document(names.len()),
            Some(Value::Object(variables)),
        ))
        .await?;
    names
        .iter()
        .enumerate()
        .map(|(i, name)| take_type(&mut data, &format!("t{i}"), name))
        .collect()
}

/// The type under `alias`, which must be the one named `name`.
fn take_type(data: &mut HashMap<String, Value>, alias: &str, name: &str) -> Result<Value> {
    let ty = data.remove(alias).unwrap_or(Value::Null);
    if ty.get("name").and_then(Value::as_str) == Some(name) {
        Ok(ty)
    } else {
        Err(Error::new(format!(
            "Unexpected introspection result: Linear listed the type {name} but did not describe it"
        )))
    }
}

fn parse(data: &Value) -> Result<cynic_introspection::Schema> {
    let unexpected = |error: &dyn std::fmt::Display| {
        Error::new(format!("Unexpected introspection result: {error}"))
    };
    let query = IntrospectionQuery::deserialize(data)
        .map_err(|error| unexpected(&error).with_source(error))?;
    query
        .into_schema()
        .map_err(|error| unexpected(&error).with_source(error))
}

/// The schema as SDL, with types, fields, arguments and enum values sorted by
/// name so the output diffs cleanly between versions.
fn sdl(mut schema: cynic_introspection::Schema) -> String {
    schema.types.sort_by(|a, b| a.name().cmp(b.name()));
    for ty in &mut schema.types {
        sort_type(ty);
    }
    // Directives the GraphQL specification defines are implied, like the
    // built-in scalars.
    schema
        .directives
        .retain(|directive| directive.name != "oneOf");
    schema.directives.sort_by(|a, b| a.name.cmp(&b.name));
    for directive in &mut schema.directives {
        directive.args.sort_by(|a, b| a.name.cmp(&b.name));
    }
    schema.to_sdl()
}

fn sort_type(ty: &mut Type) {
    match ty {
        Type::Object(object) => {
            object.interfaces.sort();
            for field in &mut object.fields {
                field.args.sort_by(|a, b| a.name.cmp(&b.name));
            }
            object.fields.sort_by(|a, b| a.name.cmp(&b.name));
        }
        Type::Interface(interface) => {
            interface.interfaces.sort();
            interface.possible_types.sort();
            for field in &mut interface.fields {
                field.args.sort_by(|a, b| a.name.cmp(&b.name));
            }
            interface.fields.sort_by(|a, b| a.name.cmp(&b.name));
        }
        Type::InputObject(input) => input.fields.sort_by(|a, b| a.name.cmp(&b.name)),
        Type::Enum(enumeration) => enumeration.values.sort_by(|a, b| a.name.cmp(&b.name)),
        Type::Union(union) => union.possible_types.sort(),
        Type::Scalar(_) => {}
    }
}
