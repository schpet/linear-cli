//! `team members`: a team's members, active ones only unless `--all`.
use cynic::QueryBuilder;
use serde::Serialize;

use super::TeamArg;
use crate::cli::team::TeamMembers;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::team_members::{self, GetTeamMembers, GetTeamMembersVariables};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;

pub fn run(ctx: &Ctx, args: &TeamMembers) -> Result<()> {
    members(ctx, args).context("Failed to list team members")
}

fn members(ctx: &Ctx, args: &TeamMembers) -> Result<()> {
    let team = TeamArg::prepare(ctx, args.team.as_deref())?;
    let client = ctx.client()?;
    let (mut members, page_info) = ctx.spin(!args.json, async {
        let key = team.key(client).await?;
        fetch(client, &key, args.all).await
    })?;
    members.sort_by(|left, right| {
        collation::compare(
            &left.display_name.to_lowercase(),
            &right.display_name.to_lowercase(),
        )
    });
    let fetched = members.len();
    if !args.all {
        members.retain(|member| member.active);
    }
    if args.json {
        ctx.print(render_json(&members, &page_info))
    } else {
        ctx.print(render_text(&members, fetched))
    }
}

/// Every member of the team, including disabled users with `all`.
async fn fetch(
    client: &GraphQlTransport,
    team_key: &str,
    all: bool,
) -> Result<(Vec<team_members::Member>, PageInfo)> {
    let result = pagination::paginate(|after| {
        let request =
            GraphQlRequest::with_variables(GetTeamMembers::build(GetTeamMembersVariables {
                team_key: team_key.to_owned(),
                include_disabled: all,
                first: Some(100),
                after,
            }));
        async move {
            let data: GetTeamMembers = client.execute(&request).await?;
            let page = data.team.members;
            Ok::<Page<team_members::Member>, Error>(Page {
                nodes: page.nodes,
                page_info: page.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more team members but returned no pagination cursor")
                .with_hint("Retry the command.")
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated a team member pagination cursor on page {page}"
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
    nodes: &'a [team_members::Member],
    page_info: &'a PageInfo,
}

fn render_json(members: &[team_members::Member], page_info: &PageInfo) -> Vec<u8> {
    let mut output = serde_json::to_vec_pretty(&JsonConnection {
        nodes: members,
        page_info,
    })
    .expect("team member JSON always serializes");
    output.push(b'\n');
    output
}

/// `fetched` counts members before the active filter, to tell an empty team
/// from one with only inactive members.
fn render_text(members: &[team_members::Member], fetched: usize) -> String {
    if fetched == 0 {
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
