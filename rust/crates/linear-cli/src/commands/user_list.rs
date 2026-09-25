//! `user list`: all-page workspace members with GraphQL-shaped JSON and text.

use std::future::Future;

use chrono::{DateTime, Local};
use cynic::QueryBuilder;
use serde::Serialize;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::organization_members::{
    self, GetOrganizationMembers, GetOrganizationMembersVariables,
};
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;

pub const CONTEXT: &str = "Failed to fetch workspace members";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: &'a [organization_members::User],
    page_info: organization_members::PageInfo,
}

pub async fn run_with<F, Fut>(
    mut fetch: F,
    include_disabled: bool,
    json: bool,
) -> Result<Vec<u8>, AppError>
where
    F: FnMut(GraphQlRequest<GetOrganizationMembersVariables>) -> Fut,
    Fut: Future<Output = Result<GetOrganizationMembers, AppError>>,
{
    let result = pagination::paginate_with_policy(pagination::EmptyCursorPolicy::Allow, |after| {
        let request = GraphQlRequest::with_variables(GetOrganizationMembers::build(
            GetOrganizationMembersVariables {
                include_disabled,
                first: Some(100),
                after,
            },
        ));
        let future = fetch(request);
        async move {
            let data = future.await?;
            let users = data.viewer.organization.users;
            Ok::<Page<organization_members::User>, AppError>(Page {
                nodes: users.nodes,
                page_info: pagination::PageInfo {
                    has_next_page: users.page_info.has_next_page,
                    end_cursor: users.page_info.end_cursor,
                },
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source.with_context(CONTEXT),
        PaginationError::MissingCursor { .. } | PaginationError::RepeatedCursor { .. } => {
            AppError::new(
                AppErrorKind::Validation,
                "Linear reported more workspace members but did not advance the page cursor",
            )
            .with_context(CONTEXT)
        }
    })?;

    let raw_count = result.nodes.len();
    let mut members = result.nodes;
    let collator = collation::root().map_err(|error| error.with_context(CONTEXT))?;
    members.sort_by(|left, right| {
        collator.compare(
            &left.display_name.to_lowercase(),
            &right.display_name.to_lowercase(),
        )
    });
    if !include_disabled {
        members.retain(|member| member.active);
    }

    if json {
        let mut output = serde_json::to_vec_pretty(&JsonConnection {
            nodes: &members,
            page_info: organization_members::PageInfo {
                has_next_page: result.page_info.has_next_page,
                end_cursor: result.page_info.end_cursor,
            },
        })
        .map_err(|error| {
            AppError::new(
                AppErrorKind::Invariant,
                "could not serialize workspace members",
            )
            .with_source(error)
            .with_context(CONTEXT)
        })?;
        output.push(b'\n');
        return Ok(output);
    }
    Ok(render_text(&members, raw_count).into_bytes())
}

pub async fn run(
    transport: &GraphQlTransport,
    include_disabled: bool,
    json: bool,
) -> Result<Vec<u8>, AppError> {
    run_with(
        |request| async move { transport.execute(&request).await.map_err(AppError::from) },
        include_disabled,
        json,
    )
    .await
}

fn truthy(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.is_empty())
}

fn local_date(value: &str) -> String {
    let Ok(parsed) = DateTime::parse_from_rfc3339(value) else {
        return "Invalid Date".to_owned();
    };
    let local = parsed.with_timezone(&Local);
    local.format("%-m/%-d/%Y, %-I:%M:%S %p").to_string()
}

pub fn render_text(members: &[organization_members::User], raw_count: usize) -> String {
    if raw_count == 0 {
        return "No members found in this workspace.\n".to_owned();
    }
    if members.is_empty() {
        return "No active members found in this workspace. Use --all to include inactive members.\n"
            .to_owned();
    }
    let mut output = format!("Workspace Members ({}):\n\n", members.len());
    for member in members {
        let header = if member.display_name.is_empty() {
            &member.name
        } else {
            &member.display_name
        };
        output.push_str(header);
        if member.name != member.display_name {
            output.push_str(&format!(" ({})", member.name));
        }
        output.push_str(&format!(" [{}]", member.initials));
        for (condition, label) in [
            (!member.active, "inactive"),
            (member.guest, "guest"),
            (!member.is_assignable, "not assignable"),
            (member.admin, "admin"),
            (member.owner, "owner"),
            (member.is_me, "you"),
        ] {
            if condition {
                output.push_str(&format!(" ({label})"));
            }
        }
        output.push('\n');
        if !member.email.is_empty() {
            output.push_str(&format!("  Email: {}\n", member.email));
        }
        if let Some(description) = truthy(member.description.as_deref()) {
            output.push_str(&format!("  Role: {description}\n"));
        }
        if let Some(timezone) = truthy(member.timezone.as_deref()) {
            output.push_str(&format!("  Timezone: {timezone}\n"));
        }
        if let (Some(emoji), Some(label)) = (
            truthy(member.status_emoji.as_deref()),
            truthy(member.status_label.as_deref()),
        ) {
            output.push_str(&format!("  Status: {emoji} {label}\n"));
        }
        if let Some(last_seen) = member
            .last_seen
            .as_ref()
            .and_then(|date| truthy(Some(&date.0)))
        {
            output.push_str(&format!("  Last seen: {}\n", local_date(last_seen)));
        }
        output.push('\n');
    }
    output
}
