//! `linear schema`: print the API's GraphQL schema as SDL or introspection JSON.
use crate::{
    cli::schema::Schema,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
};
use cynic::QueryBuilder;
use cynic_introspection::{IntrospectionQuery, Type};

pub fn run(ctx: &Ctx, args: &Schema) -> Result<()> {
    write_schema(ctx, args).context("Failed to fetch schema")
}

fn write_schema(ctx: &Ctx, args: &Schema) -> Result<()> {
    let client = ctx.client()?;
    let data: serde_json::Value = ctx.spin(true, client.execute(IntrospectionQuery::build(())))?;
    let content = if args.json {
        serde_json::to_string_pretty(&data).expect("a JSON value always serializes")
    } else {
        sdl(data)?
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

/// The schema as SDL, with types, fields, arguments and enum values sorted by
/// name so the output diffs cleanly between versions.
fn sdl(data: serde_json::Value) -> Result<String> {
    let query: IntrospectionQuery = serde_json::from_value(data).map_err(|error| {
        Error::new(format!("Unexpected introspection result: {error}")).with_source(error)
    })?;
    let mut schema = query.into_schema().map_err(|error| {
        Error::new(format!("Unexpected introspection result: {error}")).with_source(error)
    })?;
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
    Ok(schema.to_sdl())
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
