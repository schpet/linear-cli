//! `project-update list`: a project's latest status updates as a table or JSON.
use chrono::Utc;
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::project_update::ProjectUpdateList;
use crate::commands::status_update::{self, Row, UpdateHealth};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::project_updates::{
    ListProjectUpdates, ListProjectUpdatesVariables, UpdateNode, UpdateProject,
};
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
        let request = GraphQlRequest::with_variables(ListProjectUpdates::build(
            ListProjectUpdatesVariables {
                id,
                first: Some(args.limit),
            },
        ));
        let data: ListProjectUpdates = client.execute(&request).await?;
        data.project
            .ok_or_else(|| Error::not_found("Project", original))
    })?;
    if args.json {
        return ctx.print(render_json(&project));
    }
    let rows: Vec<_> = project
        .project_updates
        .nodes
        .iter()
        .map(|node| Row {
            health: node.health.as_ref().map(UpdateHealth::from),
            created_at: &node.created_at.0,
            author: author(node),
            body: &node.body,
        })
        .collect();
    if rows.is_empty() {
        return ctx.print(format!("No status updates found for {}\n", project.name));
    }
    ctx.print(status_update::table(rows, Utc::now()).render_for(ctx))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonProject<'a> {
    name: &'a str,
    slug_id: &'a str,
    project_updates: JsonConnection<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: Vec<JsonUpdate<'a>>,
    page_info: &'a crate::graphql::operations::teams::PageInfo,
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

fn render_json(project: &UpdateProject) -> Vec<u8> {
    let nodes = project
        .project_updates
        .nodes
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
    let value = JsonProject {
        name: &project.name,
        slug_id: &project.slug_id,
        project_updates: JsonConnection {
            nodes,
            page_info: &project.project_updates.page_info,
        },
    };
    let mut output =
        serde_json::to_vec_pretty(&value).expect("project update JSON always serializes");
    output.push(b'\n');
    output
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
