//! `initiative list`: typed owner resolution, complete pagination, and display.

use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{display_width, pad, truncate_text};
use crate::commands::table::underlined_header;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiatives::{
    GetInitiatives, GetInitiativesPage, GetInitiativesPageVariables, GetInitiativesVariables,
    GetViewerForInitiatives, GetViewerId, GetViewerIdVariables, IDComparator, Initiative,
    InitiativeFilter, InitiativeStatus, LookupUser, LookupUserVariables, NullableUserFilter,
};
use crate::graphql::operations::teams::{PageInfo, StringComparator};
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::{collation, opener};
use crate::refs::reject_linear_url;

pub const FETCH_CONTEXT: &str = "Failed to fetch initiatives";
pub const OPEN_CONTEXT: &str = "Failed to open initiatives";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Options {
    pub status: Option<String>,
    pub all_statuses: bool,
    pub owner: Option<String>,
    pub web: bool,
    pub app: bool,
    pub json: bool,
    pub archived: bool,
}

pub fn status_filter(status: Option<&str>, all_statuses: bool) -> Result<Option<String>, AppError> {
    match status {
        Some(value) => {
            let api = match value.to_lowercase().as_str() {
                "active" => "Active",
                "planned" => "Planned",
                "completed" => "Completed",
                _ => {
                    return Err(AppError::new(
                        AppErrorKind::Validation,
                        format!(
                            "Invalid status: {value}. Valid values: active, planned, completed"
                        ),
                    ));
                }
            };
            Ok(Some(api.to_owned()))
        }
        None if all_statuses => Ok(None),
        None => Ok(Some("Active".to_owned())),
    }
}

pub fn validate_owner(owner: Option<&str>) -> Result<(), AppError> {
    if let Some(owner) = owner {
        reject_linear_url(owner, "an email, username, display name, or @me")?;
    }
    Ok(())
}

pub fn opening(workspace: &str, app: bool) -> (String, Vec<u8>) {
    let url = format!("https://linear.app/{workspace}/initiatives");
    let destination = if app { "Linear.app" } else { "web browser" };
    let line = format!("Opening {url} in {destination}\n").into_bytes();
    (url, line)
}

pub async fn viewer_workspace(transport: &GraphQlTransport) -> Result<String, AppError> {
    let request = GraphQlRequest::without_variables(GetViewerForInitiatives::build(()));
    let result: GetViewerForInitiatives =
        transport.execute(&request).await.map_err(AppError::from)?;
    Ok(result.viewer.organization.url_key)
}

pub fn open(url: &str, app: bool) -> Result<(), AppError> {
    opener::open(url, app).map_err(|error| error.with_context(OPEN_CONTEXT))
}

pub async fn resolve_owner(
    transport: &GraphQlTransport,
    input: &str,
) -> Result<cynic::Id, AppError> {
    if input == "self" || input == "@me" {
        let request = GraphQlRequest::with_variables(GetViewerId::build(GetViewerIdVariables {}));
        let result: GetViewerId = transport.execute(&request).await.map_err(AppError::from)?;
        return Ok(result.viewer.id);
    }
    let request = GraphQlRequest::with_variables(LookupUser::build(LookupUserVariables {
        input: input.to_owned(),
    }));
    let result: LookupUser = transport.execute(&request).await.map_err(AppError::from)?;
    select_owner(&result.users.nodes, input).ok_or_else(|| AppError::not_found("Owner", input))
}

/// Shared source selection only; callers retain their own transport/error scopes.
pub fn select_owner(
    users: &[crate::graphql::operations::initiatives::LookupUserNode],
    input: &str,
) -> Option<cynic::Id> {
    let target = input.to_lowercase();
    let selected = users
        .iter()
        .find(|user| user.email.to_lowercase() == target)
        .or_else(|| {
            users
                .iter()
                .find(|user| user.display_name.to_lowercase() == target)
        })
        .or_else(|| users.first());
    selected.map(|user| user.id.clone())
}

fn filter(status: Option<&str>, owner: Option<cynic::Id>) -> Option<InitiativeFilter> {
    let status = status.map(|value| StringComparator {
        eq: Some(value.to_owned()),
        ..Default::default()
    });
    let owner = owner.map(|id| NullableUserFilter {
        id: Some(IDComparator { eq: Some(id) }),
    });
    if status.is_none() && owner.is_none() {
        None
    } else {
        Some(InitiativeFilter { status, owner })
    }
}

pub async fn run(
    transport: &GraphQlTransport,
    status: Option<&str>,
    owner: Option<&str>,
    archived: bool,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, AppError> {
    let owner_id = match owner {
        Some(input) => Some(resolve_owner(transport, input).await?),
        None => None,
    };
    let filter = filter(status, owner_id);
    let pages = pagination::paginate_with_policy(EmptyCursorPolicy::Reject, |after| {
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
                    let data: GetInitiatives =
                        transport.execute(&request).await.map_err(AppError::from)?;
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
                    let data: GetInitiativesPage =
                        transport.execute(&request).await.map_err(AppError::from)?;
                    Some(data.initiatives.ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::Invariant,
                            "Linear returned a null initiatives connection on a later page",
                        )
                    })?)
                }
            };
            let connection = connection.unwrap_or_else(|| {
                crate::graphql::operations::initiatives::InitiativeConnection {
                    nodes: Vec::new(),
                    page_info: PageInfo {
                        has_next_page: false,
                        end_cursor: None,
                    },
                }
            });
            Ok::<Page<Initiative>, AppError>(Page {
                nodes: connection.nodes,
                page_info: connection.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { page, source } => {
            if page == 1 {
                source
            } else {
                source.with_context(format!("page {page}"))
            }
        }
        PaginationError::MissingCursor { page } => AppError::new(
            AppErrorKind::Validation,
            format!(
                "Linear reported more initiatives but returned no pagination cursor on page {page}"
            ),
        )
        .with_suggestion("Retry the command."),
        PaginationError::RepeatedCursor { page, .. } => AppError::new(
            AppErrorKind::Validation,
            format!("Linear repeated an initiative pagination cursor on page {page}"),
        )
        .with_suggestion("Retry the command."),
    })?;
    let mut initiatives = pages.nodes;
    for item in &initiatives {
        if let InitiativeStatus::Unknown(value) = &item.status {
            return Err(AppError::new(
                AppErrorKind::Invariant,
                format!("Linear returned an unknown initiative status: {value}"),
            ));
        }
        if let Some(crate::graphql::operations::initiatives::InitiativeUpdateHealthType::Unknown(
            value,
        )) = &item.health
        {
            return Err(AppError::new(
                AppErrorKind::Invariant,
                format!("Linear returned an unknown initiative health: {value}"),
            ));
        }
    }
    if !initiatives.is_empty() {
        let collator = collation::root()?;
        initiatives.sort_by(|left, right| {
            left.status
                .rank()
                .cmp(&right.status.rank())
                .then_with(|| collator.compare(&left.name, &right.name))
        });
    }
    if json {
        render_json(
            &initiatives,
            &PageInfo {
                has_next_page: pages.page_info.has_next_page,
                end_cursor: pages.page_info.end_cursor,
            },
        )
    } else {
        Ok(render_text(&initiatives, columns, color).into_bytes())
    }
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
    owner: Option<&'a crate::graphql::operations::initiatives::InitiativeOwner>,
    projects: &'a crate::graphql::operations::initiatives::InitiativeProjects,
}

pub fn render_json(initiatives: &[Initiative], page_info: &PageInfo) -> Result<Vec<u8>, AppError> {
    let nodes = initiatives
        .iter()
        .map(|item| JsonInitiative {
            id: &item.id,
            slug_id: &item.slug_id,
            name: &item.name,
            description: item.description.as_deref(),
            status: item.status.as_str(),
            target_date: item.target_date.as_ref().map(|date| date.0.as_str()),
            health: item.health.as_ref().map(|health| health.as_str()),
            color: item.color.as_deref(),
            icon: item.icon.as_deref(),
            url: &item.url,
            archived_at: item.archived_at.as_ref().map(|date| date.0.as_str()),
            owner: item.owner.as_ref(),
            projects: &item.projects,
        })
        .collect();
    let mut bytes =
        serde_json::to_vec_pretty(&JsonConnection { nodes, page_info }).map_err(|error| {
            AppError::new(AppErrorKind::Invariant, "could not serialize initiatives")
                .with_source(error)
        })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub fn render_text(initiatives: &[Initiative], columns: usize, color: bool) -> String {
    if initiatives.is_empty() {
        return "No initiatives found.\n".to_owned();
    }
    let slug_width = initiatives
        .iter()
        .map(|item| display_width(&item.slug_id))
        .max()
        .unwrap_or(0)
        .max(4);
    let status_width = initiatives
        .iter()
        .map(|item| display_width(item.status.as_str()))
        .max()
        .unwrap_or(0)
        .max(6);
    let health_width = initiatives
        .iter()
        .map(|item| display_width(item.health.as_ref().map_or("-", |health| health.as_str())))
        .max()
        .unwrap_or(0)
        .max(6);
    let owner_width = initiatives
        .iter()
        .map(|item| {
            display_width(item.owner.as_ref().map_or("-", |owner| {
                if owner.initials.is_empty() {
                    "-"
                } else {
                    &owner.initials
                }
            }))
        })
        .max()
        .unwrap_or(0)
        .max(5);
    let projects_width = initiatives
        .iter()
        .map(|item| display_width(&item.projects.nodes.len().to_string()))
        .max()
        .unwrap_or(0)
        .max(4);
    let target_width = initiatives
        .iter()
        .map(|item| display_width(item.target_date.as_ref().map_or("-", |date| &date.0)))
        .max()
        .unwrap_or(0)
        .max(10);
    let fixed =
        slug_width + status_width + health_width + owner_width + projects_width + target_width + 6;
    let available = columns.saturating_sub(1 + fixed).max(10);
    let name_width = initiatives
        .iter()
        .map(|item| display_width(&item.name))
        .max()
        .unwrap_or(0)
        .min(available);
    let mut output = underlined_header(
        &[
            pad("SLUG", slug_width),
            pad("NAME", name_width),
            pad("STATUS", status_width),
            pad("HEALTH", health_width),
            pad("OWNER", owner_width),
            pad("PROJ", projects_width),
            pad("TARGET", target_width),
        ],
        color,
    );
    for item in initiatives {
        let health = item.health.as_ref().map_or("-", |value| value.as_str());
        let owner = item.owner.as_ref().map_or("-", |value| {
            if value.initials.is_empty() {
                "-"
            } else {
                &value.initials
            }
        });
        let status = item.status.as_str();
        let target = item
            .target_date
            .as_ref()
            .map_or("-", |value| value.0.as_str());
        let name = pad(&truncate_text(&item.name, name_width), name_width);
        output.push_str(&pad(&item.slug_id, slug_width));
        output.push(' ');
        output.push_str(&name);
        output.push(' ');
        if color {
            let code = match item.status {
                InitiativeStatus::Active => "\x1b[38;2;39;174;96m",
                InitiativeStatus::Planned => "\x1b[38;2;94;106;210m",
                InitiativeStatus::Completed
                | InitiativeStatus::Canceled
                | InitiativeStatus::Proposed
                | InitiativeStatus::Unknown(_) => "\x1b[38;2;107;111;118m",
            };
            output.push_str(code);
        }
        output.push_str(&pad(status, status_width));
        if color {
            output.push_str("\x1b[39m");
        }
        output.push(' ');
        output.push_str(&pad(health, health_width));
        output.push(' ');
        output.push_str(&pad(owner, owner_width));
        output.push(' ');
        output.push_str(&pad(&item.projects.nodes.len().to_string(), projects_width));
        output.push(' ');
        if color {
            output.push_str("\x1b[38;2;128;128;128m");
        }
        output.push_str(&pad(target, target_width));
        if color {
            output.push_str("\x1b[39m\x1b[0m");
        }
        output.push('\n');
    }
    output
}
