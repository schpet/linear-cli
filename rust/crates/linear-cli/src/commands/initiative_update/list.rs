//! `initiative-update list`: an initiative's latest status updates as a table or JSON.
use chrono::Utc;
use serde::Serialize;

use crate::cli::initiative_update::InitiativeUpdateList;
use crate::commands::json;
use crate::commands::status_update::{self, Row, UpdateHealth};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::status_update::{
    InitiativeUpdateNode, ListInitiativeUpdates, ListInitiativeUpdatesVariables,
};
use crate::graphql::pagination::{self, Page};
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
        pagination::collect_within(
            args.limit.max(),
            |after, first| {
                let variables = ListInitiativeUpdatesVariables {
                    id: id.clone(),
                    first: Some(first),
                    after,
                };
                async move {
                    let data: ListInitiativeUpdates = client.query(variables).await?;
                    data.initiative
                        .ok_or_else(|| Error::not_found("Initiative", original))
                }
            },
            |initiative| Page {
                nodes: std::mem::take(&mut initiative.initiative_updates.nodes),
                page_info: initiative.initiative_updates.page_info.clone(),
            },
            |initiative, page| initiative.initiative_updates.nodes = page.nodes,
        )
        .await
    })?;
    let updates = &initiative.initiative_updates.nodes;
    if args.json {
        return ctx.print(render_json(updates));
    }
    if updates.is_empty() {
        return ctx.print(format!("No status updates found for {}\n", initiative.name));
    }
    let rows = updates
        .iter()
        .map(|node| Row {
            health: Some(UpdateHealth::from(&node.health)),
            created_at: node.created_at.0,
            author: author(node),
            body: &node.body,
        })
        .collect();
    ctx.print(status_update::table(rows, Utc::now()).render_for(ctx))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonUpdate<'a> {
    id: &'a cynic::Id,
    body: &'a str,
    health: Option<&'a str>,
    url: &'a str,
    created_at: &'a crate::graphql::scalars::DateTime,
    user: Option<JsonUser<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonUser<'a> {
    name: &'a str,
    display_name: &'a str,
}

fn render_json(updates: &[InitiativeUpdateNode]) -> Vec<u8> {
    let updates: Vec<_> = updates
        .iter()
        .map(|node| JsonUpdate {
            id: &node.id,
            body: &node.body,
            health: Some(node.health.as_str()),
            url: &node.url,
            created_at: &node.created_at,
            user: node.user.as_ref().map(|user| JsonUser {
                name: &user.name,
                display_name: &user.display_name,
            }),
        })
        .collect();
    json::render(&updates)
}

fn author(node: &InitiativeUpdateNode) -> &str {
    node.user.as_ref().map_or("", |user| {
        if user.display_name.is_empty() {
            &user.name
        } else {
            &user.display_name
        }
    })
}
