//! `project-update list`: a project's latest status updates as a table or JSON.
use chrono::Utc;

use crate::cli::project_update::ProjectUpdateList;
use crate::commands::json;
use crate::commands::status_update::{self, JsonUpdate, JsonUser, Row, UpdateHealth};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::status_update::{
    ListProjectUpdates, ListProjectUpdatesVariables, ProjectUpdateNode,
};
use crate::graphql::pagination::{self, Page};
use crate::refs::{self, project::ProjectReference};

pub fn run(ctx: &Ctx, args: &ProjectUpdateList) -> Result<()> {
    list(ctx, args).context("Failed to list project updates")
}

fn list(ctx: &Ctx, args: &ProjectUpdateList) -> Result<()> {
    let original = &args.project_id;
    let reference = ProjectReference::parse(original, &ctx.scope()?)?;
    let client = ctx.client()?;
    let project = ctx.spin(!args.json, async {
        let id = refs::project::resolve(client, &reference).await?;
        pagination::collect_within(
            args.limit.max(),
            |after, first| {
                let variables = ListProjectUpdatesVariables {
                    id: id.clone(),
                    first: Some(first),
                    after,
                };
                async move {
                    let data: ListProjectUpdates = client.query(variables).await?;
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
            created_at: node.created_at.0,
            author: status_update::author(node.user.as_ref()),
            body: &node.body,
        })
        .collect();
    ctx.print(status_update::table(rows, Utc::now()).render_for(ctx))
}

fn render_json(updates: &[ProjectUpdateNode]) -> Vec<u8> {
    let updates: Vec<_> = updates
        .iter()
        .map(|node| JsonUpdate {
            id: &node.id,
            body: &node.body,
            health: node.health.as_ref().map(|health| health.as_str()),
            url: &node.url,
            created_at: &node.created_at,
            user: node.user.as_ref().map(JsonUser::from),
        })
        .collect();
    json::render(&updates)
}
