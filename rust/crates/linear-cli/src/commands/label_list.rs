//! `label list`: filter precedence, complete typed pagination and output.
use std::future::Future;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::commands::display::{display_width, fit, flexible_width, pad};
use crate::error::{Error, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::issue_labels::{
    self, GetIssueLabels, GetIssueLabelsVariables, IssueLabelFilter, NullableTeamFilter,
};
use crate::graphql::operations::teams::{self, StringComparator};
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;
use crate::refs::{
    PreparedTeamLookup, ResolvedTeam, WorkspaceScope, prepare_team_lookup,
    resolve_team_with_transport,
};

pub const CONTEXT: &str = "Failed to fetch labels";

const ID_WIDTH: usize = 36;
const COLOR_WIDTH: usize = 7;
const SPACE_WIDTH: usize = 6;
const PADDING: usize = 1;
const WORKSPACE: &str = "Workspace";

/// Local `label list` flags. `workspace_only` is the command's own
/// `--workspace` switch, distinct from the global `--workspace <slug>`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub team: Option<String>,
    pub workspace_only: bool,
    pub all: bool,
    pub json: bool,
}

/// Which labels to request once flag precedence has been applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    /// Only labels without a team.
    WorkspaceOnly,
    /// An explicit team reference, resolved to its key before the label request.
    Team(PreparedTeamLookup),
    /// The configured team key, used without a resolver lookup.
    ConfiguredTeam(String),
    /// No `filter` variable at all.
    Unfiltered,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: &'a [issue_labels::IssueLabel],
    page_info: &'a teams::PageInfo,
}

/// Pick the label scope: workspace-only, then an explicit team, then the
/// configured team unless `--all`. Call after building the client so missing
/// credentials are reported before a bad team reference.
///
/// `configured_team` is `team_key::configured_team_key`'s result: already
/// uppercased, and absent rather than empty.
pub fn select(
    options: &Options,
    configured_team: Option<&str>,
    scope: &WorkspaceScope<'_>,
) -> Result<Selection, Error> {
    if options
        .team
        .as_deref()
        .is_some_and(|team| team.trim().is_empty())
    {
        return Err(Error::new("Team reference is empty")
            .with_hint("Pass a team key, name, or ID, e.g. --team ENG.")
            .context(CONTEXT));
    }
    if options.workspace_only {
        return Ok(Selection::WorkspaceOnly);
    }
    if let Some(team) = options.team.as_deref() {
        return prepare_team_lookup(team, scope)
            .map(Selection::Team)
            .context(CONTEXT);
    }
    if options.all {
        return Ok(Selection::Unfiltered);
    }
    match configured_team {
        Some("") => Err(
            Error::new("configured team key is empty; absent keys must be None").context(CONTEXT),
        ),
        Some(key) => Ok(Selection::ConfiguredTeam(key.to_owned())),
        None => Ok(Selection::Unfiltered),
    }
}

pub async fn run_with<R, RFut, F, Fut>(
    selection: Selection,
    resolve: R,
    fetch: F,
    json: bool,
    columns: usize,
) -> Result<Vec<u8>, Error>
where
    R: FnOnce(PreparedTeamLookup) -> RFut,
    RFut: Future<Output = Result<ResolvedTeam, Error>>,
    F: FnMut(GraphQlRequest<GetIssueLabelsVariables>) -> Fut,
    Fut: Future<Output = Result<GetIssueLabels, Error>>,
{
    run_with_style(selection, resolve, fetch, json, columns, false).await
}

async fn run_with_style<R, RFut, F, Fut>(
    selection: Selection,
    resolve: R,
    mut fetch: F,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, Error>
where
    R: FnOnce(PreparedTeamLookup) -> RFut,
    RFut: Future<Output = Result<ResolvedTeam, Error>>,
    F: FnMut(GraphQlRequest<GetIssueLabelsVariables>) -> Fut,
    Fut: Future<Output = Result<GetIssueLabels, Error>>,
{
    let filter = match selection {
        Selection::WorkspaceOnly => Some(workspace_only_filter()),
        Selection::Team(prepared) => {
            let team = resolve(prepared).await.context(CONTEXT)?;
            Some(team_filter(team.key))
        }
        Selection::ConfiguredTeam(key) => Some(team_filter(key)),
        Selection::Unfiltered => None,
    };

    let result = pagination::paginate(|after| {
        let request =
            GraphQlRequest::with_variables(GetIssueLabels::build(GetIssueLabelsVariables {
                filter: filter.clone(),
                first: Some(100),
                after,
            }));
        let future = fetch(request);
        async move {
            let data = future.await?;
            Ok::<Page<issue_labels::IssueLabel>, Error>(Page {
                nodes: data.issue_labels.nodes,
                page_info: data.issue_labels.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source.context(CONTEXT),
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more labels but returned no pagination cursor")
                .with_hint("Retry the command.")
                .context(CONTEXT)
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated a label pagination cursor on page {page}"
        ))
        .with_hint("Retry the command.")
        .context(CONTEXT),
    })?;

    let mut labels = result.nodes;
    labels.sort_by(|left, right| {
        collation::compare(&left.name.to_lowercase(), &right.name.to_lowercase())
    });
    if json {
        let page_info = teams::PageInfo {
            has_next_page: result.page_info.has_next_page,
            end_cursor: result.page_info.end_cursor,
        };
        let mut output = serde_json::to_vec_pretty(&JsonConnection {
            nodes: &labels,
            page_info: &page_info,
        })
        .map_err(|error| {
            Error::new("could not serialize labels")
                .with_source(error)
                .context(CONTEXT)
        })?;
        output.push(b'\n');
        return Ok(output);
    }
    Ok(render_text(&labels, columns, color).into_bytes())
}

pub async fn run(
    transport: &GraphQlTransport,
    selection: Selection,
    json: bool,
    columns: usize,
    color: bool,
) -> Result<Vec<u8>, Error> {
    run_with_style(
        selection,
        |prepared| async move { resolve_team_with_transport(&prepared, transport).await },
        |request| async move { transport.execute(&request).await.map_err(Error::from) },
        json,
        columns,
        color,
    )
    .await
}

fn workspace_only_filter() -> IssueLabelFilter {
    IssueLabelFilter {
        team: Some(NullableTeamFilter {
            null: Some(true),
            ..NullableTeamFilter::default()
        }),
        ..IssueLabelFilter::default()
    }
}

/// A team's labels plus workspace labels, keyed by the team's key.
fn team_filter(key: String) -> IssueLabelFilter {
    IssueLabelFilter {
        or: Some(vec![
            IssueLabelFilter {
                team: Some(NullableTeamFilter {
                    key: Some(StringComparator {
                        eq: Some(key),
                        ..StringComparator::default()
                    }),
                    ..NullableTeamFilter::default()
                }),
                ..IssueLabelFilter::default()
            },
            workspace_only_filter(),
        ]),
        ..IssueLabelFilter::default()
    }
}

/// `label.team?.key || "Workspace"`: an empty key also shows `Workspace`.
fn team_display(label: &issue_labels::IssueLabel) -> &str {
    label
        .team
        .as_ref()
        .map(|team| team.key.as_str())
        .filter(|key| !key.is_empty())
        .unwrap_or(WORKSPACE)
}

pub fn render_text(labels: &[issue_labels::IssueLabel], columns: usize, color: bool) -> String {
    if labels.is_empty() {
        return "No labels found.\n".to_owned();
    }
    let team_width = labels
        .iter()
        .map(|label| display_width(team_display(label)))
        .max()
        .unwrap_or(0)
        .clamp(4, 15);
    let fixed = ID_WIDTH + COLOR_WIDTH + team_width + SPACE_WIDTH;
    let max_name_width = labels
        .iter()
        .map(|label| display_width(&label.name))
        .max()
        .unwrap_or(0);
    let available_width = columns.saturating_sub(PADDING + fixed);
    let name_width = flexible_width(max_name_width, available_width);

    let header = [
        pad("ID", ID_WIDTH),
        pad("NAME", name_width),
        pad("COLOR", COLOR_WIDTH),
        pad("TEAM", team_width),
    ];
    let mut output = if color {
        let mut line = String::new();
        for (index, cell) in header.iter().enumerate() {
            if index > 0 {
                line.push(' ');
            }
            line.push_str("\x1b[4m");
            line.push_str(cell);
            line.push_str(if index + 1 == header.len() {
                "\x1b[0m"
            } else {
                "\x1b[24m"
            });
        }
        line.push('\n');
        line
    } else {
        format!("{}\n", header.join(" "))
    };

    for label in labels {
        let id = pad(label.id.inner(), ID_WIDTH);
        let name = fit(&label.name, name_width);
        let label_color = pad(&label.color, COLOR_WIDTH);
        let team = pad(team_display(label), team_width);
        output.push_str(&format!("{id} {name} {label_color} {team}\n"));
    }
    output.push_str(&format!("\n{} labels found.\n", labels.len()));
    output
}
