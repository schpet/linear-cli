//! `user list`: every member of the workspace, as text or JSON.
use chrono::{DateTime, Local, Utc};

use crate::cli::Limit;
use crate::cli::user::UserList;
use crate::client::LinearClient;
use crate::commands::json;
use crate::commands::relative_time::ago;
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::user::{
    GetOrganizationMembers, GetOrganizationMembersVariables, User,
};
use crate::graphql::pagination::{self, Page};
use crate::platform::{collation, style};

pub fn run(ctx: &Ctx, args: &UserList) -> Result<()> {
    list(ctx, args).context("Failed to list workspace members")
}

fn list(ctx: &Ctx, args: &UserList) -> Result<()> {
    let client = ctx.client()?;
    let members = ctx.spin(!args.json, fetch(client, args.all))?;
    show(
        ctx,
        members,
        &Shown {
            all: args.all,
            limit: args.limit,
            json: args.json,
            place: "in this workspace",
        },
    )
}

/// Every member of the workspace, including disabled users with `include_disabled`.
async fn fetch(client: &LinearClient, include_disabled: bool) -> Result<Vec<User>> {
    pagination::collect(None, |after, first| {
        let variables = GetOrganizationMembersVariables {
            include_disabled,
            first: Some(first),
            after,
        };
        async move {
            let data: GetOrganizationMembers = client.query(variables).await?;
            let users = data.viewer.organization.users;
            Ok(Page {
                nodes: users.nodes,
                page_info: users.page_info,
            })
        }
    })
    .await
}

/// How `user list` and `team members` show members.
pub struct Shown {
    /// Include inactive members.
    pub all: bool,
    pub limit: Limit,
    pub json: bool,
    /// Where the members are, for the empty-list messages.
    pub place: &'static str,
}

/// Prints members by display name, without inactive ones unless `all`.
pub fn show(ctx: &Ctx, mut members: Vec<User>, shown: &Shown) -> Result<()> {
    members.sort_by(|left, right| {
        collation::compare(
            &left.display_name.to_lowercase(),
            &right.display_name.to_lowercase(),
        )
    });
    let fetched_any = !members.is_empty();
    if !shown.all {
        members.retain(|member| member.active);
    }
    shown.limit.apply(&mut members);
    let place = shown.place;
    if shown.json {
        ctx.print(json::render(&members))
    } else if !fetched_any {
        ctx.print(format!("No members found {place}.\n"))
    } else if members.is_empty() {
        ctx.print(format!(
            "No active members found {place}. Use --all to include inactive members.\n"
        ))
    } else {
        ctx.print(table(&members, Utc::now()).render_for(ctx))
    }
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
            .map(|date| ago(date.0, now, &Local))
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
