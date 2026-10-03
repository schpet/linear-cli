//! `initiative list`: every page, sorted by status then name, as a table or JSON.
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::initiative::InitiativeList;
use crate::cli::values;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiatives::{
    GetInitiatives, GetInitiativesPage, GetInitiativesPageVariables, GetInitiativesVariables,
    IDComparator, Initiative, InitiativeConnection, InitiativeFilter, InitiativeOwner,
    InitiativeProjects, InitiativeStatus, InitiativeUpdateHealthType, LookupUserNode,
    NullableUserFilter,
};
use crate::graphql::operations::teams::{PageInfo, StringComparator};
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
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
    let (initiatives, page_info) = ctx.spin(!args.json, async {
        let owner = match &args.owner {
            Some(owner) => Some(super::owner_id(client, owner).await?),
            None => None,
        };
        fetch(client, filter(status, owner), args.archived).await
    })?;
    if args.json {
        ctx.print(render_json(&initiatives, &page_info))
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

/// Every matching initiative, sorted by status then name, with the last page's info.
async fn fetch(
    client: &GraphQlTransport,
    filter: Option<InitiativeFilter>,
    archived: bool,
) -> Result<(Vec<Initiative>, PageInfo)> {
    let pages = pagination::paginate(|after| {
        let filter = filter.clone();
        async move {
            let connection = match after {
                None => {
                    let request = GraphQlRequest::with_variables(GetInitiatives::build(
                        GetInitiativesVariables {
                            filter,
                            include_archived: Some(archived),
                        },
                    ));
                    let data: GetInitiatives = client.execute(&request).await?;
                    data.initiatives
                }
                Some(after) => {
                    let request = GraphQlRequest::with_variables(GetInitiativesPage::build(
                        GetInitiativesPageVariables {
                            filter,
                            include_archived: Some(archived),
                            after: Some(after),
                        },
                    ));
                    let data: GetInitiativesPage = client.execute(&request).await?;
                    data.initiatives
                }
            };
            let connection = connection.unwrap_or_else(|| InitiativeConnection {
                nodes: Vec::new(),
                page_info: PageInfo {
                    has_next_page: false,
                    end_cursor: None,
                },
            });
            Ok::<Page<Initiative>, Error>(Page {
                nodes: connection.nodes,
                page_info: connection.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { page: 1, source } => source,
        PaginationError::Fetch { page, source } => source.context(format!("page {page}")),
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more initiatives but returned no pagination cursor")
                .with_hint("Retry the command.")
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated an initiative pagination cursor on page {page}"
        ))
        .with_hint("Retry the command."),
    })?;
    let mut initiatives = pages.nodes;
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
    let page_info = PageInfo {
        has_next_page: pages.page_info.has_next_page,
        end_cursor: pages.page_info.end_cursor,
    };
    Ok((initiatives, page_info))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: Vec<JsonInitiative<'a>>,
    page_info: &'a PageInfo,
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

fn render_json(initiatives: &[Initiative], page_info: &PageInfo) -> Vec<u8> {
    let nodes = initiatives
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
    let mut output = serde_json::to_vec_pretty(&JsonConnection { nodes, page_info })
        .expect("initiative JSON always serializes");
    output.push(b'\n');
    output
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

/// An exact email match, then an exact display name, then the first user
/// whose name contains the input.
pub fn select_owner(users: &[LookupUserNode], input: &str) -> Option<cynic::Id> {
    let target = input.to_lowercase();
    users
        .iter()
        .find(|user| user.email.to_lowercase() == target)
        .or_else(|| {
            users
                .iter()
                .find(|user| user.display_name.to_lowercase() == target)
        })
        .or_else(|| users.first())
        .map(|user| user.id.clone())
}
