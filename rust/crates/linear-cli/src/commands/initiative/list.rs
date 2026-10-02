//! `initiative list`: every page, sorted by status then name, as a table or JSON.
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::initiative::InitiativeList;
use crate::commands::display::{display_width, pad, truncate_text};
use crate::commands::table::{self, underlined_header};
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
    let status = status_filter(args.status.as_deref(), args.all_statuses)?;
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
    } else {
        let columns = table::stdout_columns(ctx.stdout_tty());
        ctx.print(render_text(&initiatives, columns, ctx.color()))
    }
}

/// The API status value to filter on: `--status`, every status with
/// `--all-statuses`, and only active initiatives otherwise.
fn status_filter(status: Option<&str>, all_statuses: bool) -> Result<Option<&'static str>> {
    match status.map(str::to_lowercase).as_deref() {
        Some("active") => Ok(Some("Active")),
        Some("planned") => Ok(Some("Planned")),
        Some("completed") => Ok(Some("Completed")),
        Some(_) => Err(Error::new(format!(
            "Invalid status: {}. Valid values: active, planned, completed",
            status.unwrap_or_default()
        ))),
        None if all_statuses => Ok(None),
        None => Ok(Some("Active")),
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
        InitiativeStatus::Planned => text.to_owned(),
        InitiativeStatus::Completed | InitiativeStatus::Proposed | InitiativeStatus::Unknown(_) => {
            style::gray(text, color)
        }
    }
}

fn render_text(initiatives: &[Initiative], columns: usize, color: bool) -> String {
    if initiatives.is_empty() {
        return "No initiatives found.\n".to_owned();
    }
    let rows: Vec<[String; 7]> = initiatives
        .iter()
        .map(|item| {
            let owner = item
                .owner
                .as_ref()
                .map(|owner| owner.initials.as_str())
                .filter(|initials| !initials.is_empty())
                .unwrap_or("-");
            [
                item.slug_id.clone(),
                item.name.clone(),
                item.status.as_str().to_owned(),
                item.health
                    .as_ref()
                    .map_or("-", InitiativeUpdateHealthType::as_str)
                    .to_owned(),
                owner.to_owned(),
                item.projects.nodes.len().to_string(),
                item.target_date
                    .as_ref()
                    .map_or("-", |date| date.0.as_str())
                    .to_owned(),
            ]
        })
        .collect();
    let headers = [
        "SLUG", "NAME", "STATUS", "HEALTH", "OWNER", "PROJ", "TARGET",
    ];
    let mut widths = [4, 0, 6, 6, 5, 4, 10];
    for row in &rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(display_width(cell));
        }
    }
    let fixed: usize = widths.iter().sum::<usize>() - widths[1] + widths.len() - 1;
    widths[1] = widths[1].min(columns.saturating_sub(1 + fixed).max(10));
    let header: Vec<String> = headers
        .iter()
        .zip(widths)
        .map(|(header, width)| pad(header, width))
        .collect();
    let mut output = underlined_header(&header, color);
    for (item, row) in initiatives.iter().zip(&rows) {
        let name = pad(&truncate_text(&row[1], widths[1]), widths[1]);
        let status = status_style(&item.status, &pad(&row[2], widths[2]), color);
        let target = style::gray(&pad(&row[6], widths[6]), color);
        output.push_str(&format!(
            "{} {name} {status} {} {} {} {target}\n",
            pad(&row[0], widths[0]),
            pad(&row[3], widths[3]),
            pad(&row[4], widths[4]),
            pad(&row[5], widths[5]),
        ));
    }
    output
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
