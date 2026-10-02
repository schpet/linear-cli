//! `label list`: a team's labels plus workspace labels, or every label, as a
//! table or JSON.
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::label::LabelList;
use crate::commands::display::{display_width, fit, flexible_width, pad};
use crate::commands::table;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::issue_labels::{
    GetIssueLabels, GetIssueLabelsVariables, IssueLabel, IssueLabelFilter, NullableTeamFilter,
};
use crate::graphql::operations::teams::{PageInfo, StringComparator};
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::{collation, style};
use crate::refs::{PreparedTeamLookup, prepare_team_lookup, resolve_team_with_transport};

const ID_WIDTH: usize = 36;
const COLOR_WIDTH: usize = 7;
const SPACE_WIDTH: usize = 6;
const PADDING: usize = 1;
const WORKSPACE: &str = "Workspace";

/// Which labels to list.
enum Scope {
    /// Only labels without a team.
    Workspace,
    /// A team's labels plus workspace labels.
    Team(PreparedTeamLookup),
    All,
}

pub fn run(ctx: &Ctx, args: &LabelList) -> Result<()> {
    list(ctx, args).context("Failed to list labels")
}

fn list(ctx: &Ctx, args: &LabelList) -> Result<()> {
    let scope = if args.workspace_only {
        Scope::Workspace
    } else {
        // --team, then the configured team unless --all.
        let team = match &args.team {
            Some(team) => Some(team.clone()),
            None if args.all => None,
            None => configured_team_key(ctx.options()),
        };
        match team {
            Some(team) => Scope::Team(prepare_team_lookup(&team, &ctx.scope()?)?),
            None => Scope::All,
        }
    };
    let client = ctx.client()?;
    let (mut labels, page_info) = ctx.spin(!args.json, async {
        let filter = match &scope {
            Scope::Workspace => Some(workspace_only_filter()),
            Scope::Team(lookup) => {
                let team = resolve_team_with_transport(lookup, client).await?;
                Some(team_filter(team.key))
            }
            Scope::All => None,
        };
        fetch(client, filter).await
    })?;
    labels.sort_by(|left, right| {
        collation::compare(&left.name.to_lowercase(), &right.name.to_lowercase())
    });
    if args.json {
        ctx.print(render_json(&labels, &page_info))
    } else {
        let columns = table::stdout_columns(ctx.stdout_tty());
        ctx.print(render_text(&labels, columns, ctx.color()))
    }
}

/// Every page of labels matching `filter`, with the last page's info.
async fn fetch(
    client: &GraphQlTransport,
    filter: Option<IssueLabelFilter>,
) -> Result<(Vec<IssueLabel>, PageInfo)> {
    let result = pagination::paginate(|after| {
        let request =
            GraphQlRequest::with_variables(GetIssueLabels::build(GetIssueLabelsVariables {
                filter: filter.clone(),
                first: Some(100),
                after,
            }));
        async move {
            let data: GetIssueLabels = client.execute(&request).await?;
            Ok::<Page<IssueLabel>, Error>(Page {
                nodes: data.issue_labels.nodes,
                page_info: data.issue_labels.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more labels but returned no pagination cursor")
                .with_hint("Retry the command.")
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated a label pagination cursor on page {page}"
        ))
        .with_hint("Retry the command."),
    })?;
    let page_info = PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };
    Ok((result.nodes, page_info))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: &'a [IssueLabel],
    page_info: &'a PageInfo,
}

fn render_json(labels: &[IssueLabel], page_info: &PageInfo) -> Vec<u8> {
    let mut output = serde_json::to_vec_pretty(&JsonConnection {
        nodes: labels,
        page_info,
    })
    .expect("label JSON always serializes");
    output.push(b'\n');
    output
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

fn team_display(label: &IssueLabel) -> &str {
    label
        .team
        .as_ref()
        .map(|team| team.key.as_str())
        .filter(|key| !key.is_empty())
        .unwrap_or(WORKSPACE)
}

fn render_text(labels: &[IssueLabel], columns: usize, color: bool) -> String {
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
    let mut output = format!("{}\n", style::underline(&header.join(" "), color));

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
