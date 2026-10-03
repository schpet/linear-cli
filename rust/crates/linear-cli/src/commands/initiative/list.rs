//! `initiative list`: every page, sorted by status then name, as a table or JSON.
use serde::Serialize;

use crate::cli::initiative::InitiativeList;
use crate::cli::values;
use crate::client::LinearClient;
use crate::commands::table::{Cell, Column, Table};
use crate::commands::{json, user};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::initiative::{
    GetInitiatives, GetInitiativesVariables, IDComparator, Initiative, InitiativeFilter,
    InitiativeOwner, InitiativeProjects, InitiativeStatus, InitiativeUpdateHealthType,
    NullableUserFilter,
};
use crate::graphql::operations::team::StringComparator;
use crate::graphql::pagination::{self, Page, PageInfo};
use crate::platform::{collation, style};

pub fn run(ctx: &Ctx, args: &InitiativeList) -> Result<()> {
    if args.web || args.app {
        return ctx
            .open_in_linear("initiatives", args.app)
            .context("Failed to open initiatives");
    }
    list(ctx, args).context("Failed to list initiatives")
}

fn list(ctx: &Ctx, args: &InitiativeList) -> Result<()> {
    let status = status_filter(args.status, args.all_statuses);
    super::check_owner(args.owner.as_deref())?;
    let client = ctx.client()?;
    let mut initiatives = ctx.spin(!args.json, async {
        let owner = match &args.owner {
            Some(owner) => Some(user::resolve(client, owner, "Owner").await?),
            None => None,
        };
        fetch(client, filter(status, owner), args.archived).await
    })?;
    args.limit.apply(&mut initiatives);
    if args.json {
        ctx.print(render_json(&initiatives))
    } else if initiatives.is_empty() {
        ctx.print("No initiatives found.\n")
    } else {
        ctx.print(render_text(&initiatives).render_for(ctx))
    }
}

/// The API status value to filter on: `--status`, every status with
/// `--all-statuses`, and only active initiatives otherwise.
fn status_filter(
    status: Option<values::InitiativeStatus>,
    all_statuses: bool,
) -> Option<&'static str> {
    match status {
        Some(values::InitiativeStatus::Active) => Some("Active"),
        Some(values::InitiativeStatus::Planned) => Some("Planned"),
        Some(values::InitiativeStatus::Completed) => Some("Completed"),
        None if all_statuses => None,
        None => Some("Active"),
    }
}

fn filter(status: Option<&str>, owner: Option<String>) -> Option<InitiativeFilter> {
    let status = status.map(|value| StringComparator {
        eq: Some(value.to_owned()),
        ..Default::default()
    });
    let owner = owner.map(|id| NullableUserFilter {
        id: Some(IDComparator {
            eq: Some(cynic::Id::new(id)),
        }),
    });
    (status.is_some() || owner.is_some()).then_some(InitiativeFilter { status, owner })
}

/// Every matching initiative, sorted by status then name.
async fn fetch(
    client: &LinearClient,
    filter: Option<InitiativeFilter>,
    archived: bool,
) -> Result<Vec<Initiative>> {
    let mut initiatives = pagination::collect(None, |after, first| {
        let variables = GetInitiativesVariables {
            filter: filter.clone(),
            include_archived: Some(archived),
            first: Some(first),
            after,
        };
        async move {
            let data: GetInitiatives = client.query(variables).await?;
            Ok(data.initiatives.map_or_else(
                || Page {
                    nodes: Vec::new(),
                    page_info: PageInfo {
                        has_next_page: false,
                        end_cursor: None,
                    },
                },
                |connection| Page {
                    nodes: connection.nodes,
                    page_info: connection.page_info,
                },
            ))
        }
    })
    .await?;
    for item in &initiatives {
        if let InitiativeStatus::Unknown(value) = &item.status {
            return Err(Error::new(format!(
                "Linear returned an unknown initiative status: {value}"
            )));
        }
        if let Some(InitiativeUpdateHealthType::Unknown(value)) = &item.health {
            return Err(Error::new(format!(
                "Linear returned an unknown initiative health: {value}"
            )));
        }
    }
    initiatives.sort_by(|left, right| {
        left.status
            .rank()
            .cmp(&right.status.rank())
            .then_with(|| collation::compare(&left.name, &right.name))
    });
    Ok(initiatives)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonInitiative<'a> {
    id: &'a cynic::Id,
    slug_id: &'a str,
    name: &'a str,
    description: Option<&'a str>,
    status: &'a str,
    target_date: Option<&'a str>,
    health: Option<&'a str>,
    color: Option<&'a str>,
    icon: Option<&'a str>,
    url: &'a str,
    archived_at: Option<&'a str>,
    owner: Option<&'a InitiativeOwner>,
    projects: &'a InitiativeProjects,
}

fn render_json(initiatives: &[Initiative]) -> Vec<u8> {
    let initiatives: Vec<_> = initiatives
        .iter()
        .map(|item| JsonInitiative {
            id: &item.id,
            slug_id: &item.slug_id,
            name: &item.name,
            description: item.description.as_deref(),
            status: item.status.as_str(),
            target_date: item.target_date.as_ref().map(|date| date.0.as_str()),
            health: item.health.as_ref().map(InitiativeUpdateHealthType::as_str),
            color: item.color.as_deref(),
            icon: item.icon.as_deref(),
            url: &item.url,
            archived_at: item.archived_at.as_ref().map(|date| date.0.as_str()),
            owner: item.owner.as_ref(),
            projects: &item.projects,
        })
        .collect();
    json::render(&initiatives)
}

/// The status column's color, matching the status colors in Linear.
pub(super) fn status_style(status: &InitiativeStatus, text: &str, color: bool) -> String {
    match status {
        InitiativeStatus::Active => style::green(text, color),
        InitiativeStatus::Canceled => style::red(text, color),
        InitiativeStatus::Planned => style::blue(text, color),
        InitiativeStatus::Completed | InitiativeStatus::Proposed | InitiativeStatus::Unknown(_) => {
            style::gray(text, color)
        }
    }
}

fn render_text(initiatives: &[Initiative]) -> Table {
    let mut table = Table::new([
        Column::fixed("SLUG"),
        Column::flexible("NAME"),
        Column::fixed("STATUS"),
        Column::fixed("HEALTH"),
        Column::fixed("OWNER"),
        Column::fixed("PROJ"),
        Column::fixed("TARGET"),
    ]);
    for item in initiatives {
        let owner = item
            .owner
            .as_ref()
            .map(|owner| owner.initials.as_str())
            .filter(|initials| !initials.is_empty())
            .unwrap_or("-");
        let status = item.status.clone();
        table.row([
            Cell::from(item.slug_id.as_str()),
            Cell::from(item.name.as_str()),
            Cell::styled(item.status.as_str(), move |text, on| {
                status_style(&status, text, on)
            }),
            Cell::from(
                item.health
                    .as_ref()
                    .map_or("-", InitiativeUpdateHealthType::as_str),
            ),
            Cell::from(owner),
            Cell::from(item.projects.nodes.len().to_string()),
            Cell::styled(
                item.target_date
                    .as_ref()
                    .map_or("-", |date| date.0.as_str()),
                style::gray,
            ),
        ]);
    }
    table
}
