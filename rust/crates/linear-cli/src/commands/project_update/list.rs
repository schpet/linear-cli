//! `project-update list`: a project's latest status updates as a table or JSON.
use chrono::Utc;
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::project_update::ProjectUpdateList;
use crate::commands::json;
use crate::commands::status_update::{self, Row, UpdateHealth};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::LegacyRequest;
use crate::graphql::operations::project_updates::{
    ListProjectUpdates, ListProjectUpdatesVariables, UpdateNode,
};
use crate::graphql::pagination::{self, Page};
use crate::refs::{prepare_project_lookup, resolve_project_with_transport};

pub fn run(ctx: &Ctx, args: &ProjectUpdateList) -> Result<()> {
    list(ctx, args).context("Failed to list project updates")
}

fn list(ctx: &Ctx, args: &ProjectUpdateList) -> Result<()> {
    let original = &args.project_id;
    let reference = prepare_project_lookup(original, &ctx.scope()?)?;
    let client = ctx.client()?;
    let project = ctx.spin(!args.json, async {
        let id = resolve_project_with_transport(&reference, original, client).await?;
        pagination::collect_within(
            args.limit.max(),
            |after, first| {
                let request = LegacyRequest::with_variables(ListProjectUpdates::build(
                    ListProjectUpdatesVariables {
                        id: id.clone(),
                        first: Some(first),
                        after,
                    },
                ));
                async move {
                    let data: ListProjectUpdates = client.execute_legacy(&request).await?;
                    data.project
                        .ok_or_else(|| Error::not_found("Project", original))
                }
            },
            |project| Page {
                nodes: std::mem::take(&mut project.project_updates.nodes),
                page_info: project.project_updates.page_info.clone(),
            },
            |project, page| project.project_updates.nodes = page.nodes,
        )
        .await
    })?;
    let updates = &project.project_updates.nodes;
    if args.json {
        return ctx.print(render_json(updates));
    }
    if updates.is_empty() {
        return ctx.print(format!("No status updates found for {}\n", project.name));
    }
    let rows = updates
        .iter()
        .map(|node| Row {
            health: node.health.as_ref().map(UpdateHealth::from),
            created_at: &node.created_at.0,
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

fn render_json(updates: &[UpdateNode]) -> Vec<u8> {
    let updates: Vec<_> = updates
        .iter()
        .map(|node| JsonUpdate {
            id: &node.id,
            body: &node.body,
            health: node.health.as_ref().map(|health| health.as_str()),
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

fn author(node: &UpdateNode) -> &str {
    node.user.as_ref().map_or("", |user| {
        if user.display_name.is_empty() {
            &user.name
        } else {
            &user.display_name
        }
    })
}
