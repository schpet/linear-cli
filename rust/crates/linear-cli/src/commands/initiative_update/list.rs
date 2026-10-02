//! `initiative-update list`: an initiative's latest status updates as a table or JSON.
use chrono::Utc;
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::initiative_update::InitiativeUpdateList;
use crate::commands::status_update::{self, Row, UpdateHealth};
use crate::commands::table;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiative_updates::{
    ListInitiativeUpdates, ListInitiativeUpdatesVariables, UpdateInitiative,
};
use crate::refs::{prepare_initiative_lookup, resolve_initiative_with_transport};

pub fn run(ctx: &Ctx, args: &InitiativeUpdateList) -> Result<()> {
    list(ctx, args).context("Failed to list initiative updates")
}

fn list(ctx: &Ctx, args: &InitiativeUpdateList) -> Result<()> {
    let original = &args.initiative_id;
    let reference = prepare_initiative_lookup(original, &ctx.scope()?)?;
    let client = ctx.client()?;
    let initiative = ctx.spin(!args.json, async {
        let id = resolve_initiative_with_transport(&reference, original, client).await?;
        let request = GraphQlRequest::with_variables(ListInitiativeUpdates::build(
            ListInitiativeUpdatesVariables {
                id,
                first: Some(args.limit),
            },
        ));
        let data: ListInitiativeUpdates = client.execute(&request).await?;
        data.initiative
            .ok_or_else(|| Error::not_found("Initiative", original))
    })?;
    if args.json {
        return ctx.print(render_json(&initiative));
    }
    let rows: Vec<_> = initiative
        .initiative_updates
        .nodes
        .iter()
        .map(|node| Row {
            id: node.id.inner(),
            health: Some(UpdateHealth::from(&node.health)),
            created_at: &node.created_at.0,
            author: node.user.as_ref().map_or("", |user| user.name.as_str()),
            body: &node.body,
        })
        .collect();
    ctx.print(status_update::render_list(
        &initiative.name,
        &rows,
        table::stdout_columns(ctx.stdout_tty()),
        ctx.color(),
        Utc::now(),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonInitiative<'a> {
    name: &'a str,
    slug_id: &'a str,
    initiative_updates: JsonConnection<'a>,
}

#[derive(Serialize)]
struct JsonConnection<'a> {
    nodes: Vec<JsonUpdate<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonUpdate<'a> {
    id: &'a cynic::Id,
    body: &'a str,
    health: &'a str,
    url: &'a str,
    created_at: &'a crate::graphql::scalars::DateTime,
    user: Option<JsonUser<'a>>,
}

#[derive(Serialize)]
struct JsonUser<'a> {
    name: &'a str,
}

fn render_json(initiative: &UpdateInitiative) -> Vec<u8> {
    let value = JsonInitiative {
        name: &initiative.name,
        slug_id: &initiative.slug_id,
        initiative_updates: JsonConnection {
            nodes: initiative
                .initiative_updates
                .nodes
                .iter()
                .map(|node| JsonUpdate {
                    id: &node.id,
                    body: &node.body,
                    health: node.health.as_str(),
                    url: &node.url,
                    created_at: &node.created_at,
                    user: node.user.as_ref().map(|user| JsonUser { name: &user.name }),
                })
                .collect(),
        },
    };
    let mut output =
        serde_json::to_vec_pretty(&value).expect("initiative update JSON always serializes");
    output.push(b'\n');
    output
}
