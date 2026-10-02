//! `team members`: typed pages, local active filter, and member display.
use std::future::Future;

use cynic::QueryBuilder;
use serde::Serialize;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::team_members::{self, GetTeamMembers, GetTeamMembersVariables};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, EmptyCursorPolicy, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;

pub const CONTEXT: &str = "Failed to fetch team members";
const CURSOR_ERROR: &str = "Linear reported more team members but did not advance the page cursor";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Options {
    pub all: bool,
    pub json: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: &'a [team_members::Member],
    page_info: &'a PageInfo,
}

fn cursor_error() -> AppError {
    AppError::new(AppErrorKind::Validation, CURSOR_ERROR)
}

pub async fn run_with<F, Fut>(
    mut fetch: F,
    team_key: &str,
    options: Options,
) -> Result<Vec<u8>, AppError>
where
    F: FnMut(GraphQlRequest<GetTeamMembersVariables>) -> Fut,
    Fut: Future<Output = Result<GetTeamMembers, AppError>>,
{
    let result = pagination::paginate_with_policy(EmptyCursorPolicy::Allow, |after| {
        let request =
            GraphQlRequest::with_variables(GetTeamMembers::build(GetTeamMembersVariables {
                team_key: team_key.to_owned(),
                include_disabled: options.all,
                first: Some(100),
                after,
            }));
        let future = fetch(request);
        async move {
            let data = future.await?;
            let page = data.team.members;
            Ok::<Page<team_members::Member>, AppError>(Page {
                nodes: page.nodes,
                page_info: page.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source.with_context(CONTEXT),
        PaginationError::MissingCursor { .. } | PaginationError::RepeatedCursor { .. } => {
            cursor_error().with_context(CONTEXT)
        }
    })?;
    let mut nodes = result.nodes;
    let page_info = PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };

    let collator = collation::root().map_err(|error| error.with_context(CONTEXT))?;
    nodes.sort_by(|left, right| {
        collator.compare(
            &left.display_name.to_lowercase(),
            &right.display_name.to_lowercase(),
        )
    });
    let source_count = nodes.len();
    if !options.all {
        nodes.retain(|member| member.active);
    }
    if options.json {
        let mut output = serde_json::to_vec_pretty(&JsonConnection {
            nodes: &nodes,
            page_info: &page_info,
        })
        .map_err(|error| {
            AppError::new(AppErrorKind::Invariant, "could not serialize team members")
                .with_source(error)
                .with_context(CONTEXT)
        })?;
        output.push(b'\n');
        return Ok(output);
    }
    Ok(render_text(&nodes, source_count).into_bytes())
}

pub async fn run(
    transport: &GraphQlTransport,
    team_key: &str,
    options: Options,
) -> Result<Vec<u8>, AppError> {
    run_with(
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
        team_key,
        options,
    )
    .await
}

pub fn render_text(members: &[team_members::Member], source_count: usize) -> String {
    if source_count == 0 {
        return "No members found for this team.\n".to_owned();
    }
    if members.is_empty() {
        return "No active members found for this team. Use --all to include inactive members.\n"
            .to_owned();
    }
    let mut output = format!("Team Members ({}):\n\n", members.len());
    for member in members {
        let display = if member.display_name.is_empty() {
            &member.name
        } else {
            &member.display_name
        };
        output.push_str(display);
        if member.name != member.display_name {
            output.push_str(&format!(" ({})", member.name));
        }
        output.push_str(&format!(" [{}]", member.initials));
        if !member.active {
            output.push_str(" (inactive)");
        }
        if member.guest {
            output.push_str(" (guest)");
        }
        if !member.is_assignable {
            output.push_str(" (not assignable)");
        }
        if member.admin {
            output.push_str(" (admin)");
        }
        if member.owner {
            output.push_str(" (owner)");
        }
        if member.is_me {
            output.push_str(" (you)");
        }
        output.push('\n');
        if !member.email.is_empty() {
            output.push_str(&format!("  Email: {}\n", member.email));
        }
        if let Some(description) = member.description.as_ref().filter(|text| !text.is_empty()) {
            output.push_str(&format!("  Role: {description}\n"));
        }
        if let Some(timezone) = member.timezone.as_ref().filter(|text| !text.is_empty()) {
            output.push_str(&format!("  Timezone: {timezone}\n"));
        }
        if let (Some(emoji), Some(label)) = (
            member.status_emoji.as_ref().filter(|text| !text.is_empty()),
            member.status_label.as_ref().filter(|text| !text.is_empty()),
        ) {
            output.push_str(&format!("  Status: {emoji} {label}\n"));
        }
        if let Some(last_seen) = member.last_seen.as_ref().filter(|date| !date.0.is_empty()) {
            output.push_str(&format!(
                "  Last seen: {}\n",
                crate::commands::relative_time::format_local_timestamp(&last_seen.0)
            ));
        }
        output.push('\n');
    }
    output
}
