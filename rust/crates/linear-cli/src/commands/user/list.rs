//! `user list`: every member of the workspace, as text or JSON.
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::user::UserList;
use crate::commands::relative_time::format_local_timestamp;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::organization_members::{
    GetOrganizationMembers, GetOrganizationMembersVariables, PageInfo, User,
};
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;

pub fn run(ctx: &Ctx, args: &UserList) -> Result<()> {
    list(ctx, args).context("Failed to list workspace members")
}

fn list(ctx: &Ctx, args: &UserList) -> Result<()> {
    let client = ctx.client()?;
    let (fetched, page_info) = ctx.spin(!args.json, fetch(client, args.all))?;
    let fetched_any = !fetched.is_empty();
    let mut members = fetched;
    members.sort_by(|left, right| {
        collation::compare(
            &left.display_name.to_lowercase(),
            &right.display_name.to_lowercase(),
        )
    });
    if !args.all {
        members.retain(|member| member.active);
    }
    if args.json {
        ctx.print(render_json(&members, page_info))
    } else {
        ctx.print(render_text(&members, fetched_any))
    }
}

/// Every page of members, with the last page's info.
async fn fetch(client: &GraphQlTransport, include_disabled: bool) -> Result<(Vec<User>, PageInfo)> {
    let result = pagination::paginate(|after| {
        let request = GraphQlRequest::with_variables(GetOrganizationMembers::build(
            GetOrganizationMembersVariables {
                include_disabled,
                first: Some(100),
                after,
            },
        ));
        async move {
            let data: GetOrganizationMembers = client.execute(&request).await?;
            let users = data.viewer.organization.users;
            Ok::<Page<User>, Error>(Page {
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
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } | PaginationError::RepeatedCursor { .. } => {
            Error::new("Linear reported more workspace members but did not advance the page cursor")
                .with_hint("Retry the command.")
        }
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
    nodes: &'a [User],
    page_info: PageInfo,
}

fn render_json(members: &[User], page_info: PageInfo) -> Vec<u8> {
    let mut output = serde_json::to_vec_pretty(&JsonConnection {
        nodes: members,
        page_info,
    })
    .expect("member JSON always serializes");
    output.push(b'\n');
    output
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.is_empty())
}

/// `fetched_any` tells an empty workspace apart from one whose members are
/// all inactive (and filtered out).
fn render_text(members: &[User], fetched_any: bool) -> String {
    if !fetched_any {
        return "No members found in this workspace.\n".to_owned();
    }
    if members.is_empty() {
        return "No active members found in this workspace. Use --all to include inactive members.\n"
            .to_owned();
    }
    let mut output = format!("Workspace Members ({}):\n\n", members.len());
    for member in members {
        if member.display_name.is_empty() || member.display_name == member.name {
            output.push_str(&member.name);
        } else {
            output.push_str(&format!("{} ({})", member.display_name, member.name));
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
        if let Some(description) = nonempty(member.description.as_deref()) {
            output.push_str(&format!("  Role: {description}\n"));
        }
        if let Some(timezone) = nonempty(member.timezone.as_deref()) {
            output.push_str(&format!("  Timezone: {timezone}\n"));
        }
        if let (Some(emoji), Some(label)) = (
            nonempty(member.status_emoji.as_deref()),
            nonempty(member.status_label.as_deref()),
        ) {
            output.push_str(&format!("  Status: {emoji} {label}\n"));
        }
        if let Some(last_seen) = member
            .last_seen
            .as_ref()
            .and_then(|date| nonempty(Some(&date.0)))
        {
            output.push_str(&format!(
                "  Last seen: {}\n",
                format_local_timestamp(last_seen)
            ));
        }
        output.push('\n');
    }
    output
}
