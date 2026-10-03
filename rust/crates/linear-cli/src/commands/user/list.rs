//! `user list`: every member of the workspace, as text or JSON.
use chrono::{DateTime, Local, Utc};
use cynic::QueryBuilder;
use serde::Serialize;

use crate::cli::user::UserList;
use crate::commands::relative_time::format_relative_time;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::organization_members::{
    GetOrganizationMembers, GetOrganizationMembersVariables, PageInfo, User,
};
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::{collation, style};

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
    } else if !fetched_any {
        ctx.print("No members found in this workspace.\n")
    } else if members.is_empty() {
        ctx.print(
            "No active members found in this workspace. Use --all to include inactive members.\n",
        )
    } else {
        ctx.print(table(&members, Utc::now()).render_for(ctx))
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

/// Workspace or team members, one row each.
pub fn table(members: &[User], now: DateTime<Utc>) -> Table {
    let mut table = Table::new([
        Column::flexible("NAME"),
        Column::fixed("USERNAME"),
        Column::flexible("EMAIL"),
        Column::fixed("ROLE"),
        Column::fixed("LAST SEEN"),
    ]);
    for member in members {
        let mut name = member.name.clone();
        if member.is_me {
            name.push_str(" (you)");
        }
        if !member.active {
            name.push_str(" (inactive)");
        }
        let role = if member.owner {
            "Owner"
        } else if member.admin {
            "Admin"
        } else if member.guest {
            "Guest"
        } else {
            "Member"
        };
        let last_seen = member
            .last_seen
            .as_ref()
            .map(|date| format_relative_time(&date.0, now, &Local))
            .unwrap_or_default();
        let name = if member.active {
            Cell::from(name)
        } else {
            Cell::styled(name, style::gray)
        };
        table.row([
            name,
            Cell::from(member.display_name.as_str()),
            Cell::from(member.email.as_str()),
            Cell::from(role),
            Cell::styled(last_seen, style::gray),
        ]);
    }
    table
}
